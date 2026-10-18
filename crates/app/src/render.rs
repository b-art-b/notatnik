//! Markdown → [`Node`] tree for the UI (decision D15/D17).
//!
//! Our own syntax (links, tags, block refs, embeds) is found by `notatnik-core` and replaced by a
//! single private-use character before the text goes through `pulldown-cmark`, so it can't be
//! confused with emphasis or link syntax. Raw HTML is never rendered: it comes out as literal text.

use std::iter::Peekable;

use notatnik_core::RefKind;
use pulldown_cmark::{Alignment, CodeBlockKind, Event, Options, Parser, Tag, TagEnd};

use crate::dto::{Inline, Node};

const PUA_BASE: u32 = 0xE000;
const PUA_MAX: usize = 0x1800;

/// A reference inside the text being rendered (offsets are into that text).
#[derive(Debug, Clone)]
pub struct RefSpan {
    pub start: usize,
    pub end: usize,
    pub kind: RefKind,
    pub target: String,
    pub label: Option<String>,
}

fn opts() -> Options {
    Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_FOOTNOTES
}

thread_local! {
    /// The folder of the page being rendered (graph-relative, `pages` or `journals/2026`), so that relative links
    /// and images such as `.attachments.12/a.png` can be turned into paths from the graph's root.
    static BASE_DIR: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
}

/// Puts the page's folder in force for [`render`] until the guard is dropped.
pub struct BaseDir(Option<String>);

/// Renders with relative links and images resolved against `dir` (the folder of the page file).
pub fn with_base_dir(dir: &str) -> BaseDir {
    BaseDir(BASE_DIR.with(|b| b.replace(Some(dir.to_string()))))
}

impl Drop for BaseDir {
    fn drop(&mut self) {
        BASE_DIR.with(|b| b.replace(self.0.take()));
    }
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            let hex = |c: u8| (c as char).to_digit(16);
            if let (Some(h), Some(l)) = (hex(b[i + 1]), hex(b[i + 2])) {
                out.push((h * 16 + l) as u8);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// `rel` (as written in a page of folder `dir`) as a path from the graph's root, or `None` when it leaves the
/// graph or points into `.notatnik`.
fn resolve_in_graph(dir: &str, rel: &str) -> Option<String> {
    let rel = rel.split(['?', '#']).next().unwrap_or("");
    if rel.is_empty() || rel.starts_with('/') {
        return None;
    }
    let mut parts: Vec<String> = dir
        .split('/')
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect();
    for seg in percent_decode(rel).split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            s => parts.push(s.to_string()),
        }
    }
    if parts.is_empty() || parts.first().is_some_and(|p| p == ".notatnik") {
        return None;
    }
    Some(parts.join("/"))
}

fn is_image_path(path: &str) -> bool {
    let ext = path
        .rsplit_once('.')
        .map_or("", |(_, e)| e)
        .to_ascii_lowercase();
    matches!(
        ext.as_str(),
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "avif" | "bmp"
    )
}

/// Where a relative link or image points, from the graph's root. Without a page folder in force (a list of
/// results, a test) only `assets/…` paths are understood, as written.
fn local_target(dest: &str, image: bool) -> Option<String> {
    if has_scheme(dest) || dest.trim().is_empty() || dest.trim_start().starts_with('#') {
        return None;
    }
    let base = BASE_DIR.with(|b| b.borrow().clone());
    match base {
        Some(dir) => {
            let path = resolve_in_graph(&dir, dest.trim())?;
            // a link to another page (`other.md`) is not a file to open
            let page = path.rsplit_once('.').is_some_and(|(_, e)| {
                e.eq_ignore_ascii_case("md") || e.eq_ignore_ascii_case("markdown")
            });
            (image || !page).then_some(path)
        }
        None => (image || is_asset_link(dest)).then(|| dest.to_string()),
    }
}

/// Renders block content.
pub fn render(content: &str, refs: &[RefSpan]) -> Vec<Node> {
    let mut text = String::with_capacity(content.len());
    let mut last = 0;
    let mut used: Vec<&RefSpan> = Vec::new();
    for r in refs.iter().take(PUA_MAX) {
        if r.start < last
            || r.end > content.len()
            || !content.is_char_boundary(r.start)
            || !content.is_char_boundary(r.end)
        {
            continue;
        }
        text.push_str(content.get(last..r.start).unwrap_or(""));
        text.push(char::from_u32(PUA_BASE + used.len() as u32).unwrap_or('\u{fffd}'));
        used.push(r);
        last = r.end;
    }
    text.push_str(content.get(last..).unwrap_or(""));
    let lines: Vec<&str> = text.split('\n').collect();
    parse_lines(&lines, &used, 0)
}

/// Containers other editors write around Markdown: `::: info` ... `:::` and `<details>` ... `</details>`.
/// They are found line by line (outside code fences) and their insides are rendered again, so they can
/// hold anything a block can. An opener without a closer stays ordinary text.
const MAX_DEPTH: usize = 8;

enum Open {
    Callout(String),
    Details { open: bool, rest: String },
}

fn opener(line: &str) -> Option<Open> {
    let t = line.trim();
    if let Some(rest) = t.strip_prefix(":::") {
        let rest = rest.trim_start_matches(':').trim();
        let kind = rest.split_whitespace().next()?;
        if kind
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return Some(Open::Callout(kind.to_ascii_lowercase()));
        }
        return None;
    }
    let tag = t.strip_prefix("<details")?;
    let end = tag.find('>')?;
    let attrs = &tag[..end];
    if !(attrs.is_empty() || attrs.starts_with(char::is_whitespace)) {
        return None;
    }
    Some(Open::Details {
        open: attrs
            .split_whitespace()
            .any(|a| a.eq_ignore_ascii_case("open")),
        rest: tag[end + 1..].to_string(),
    })
}

fn is_close(open: &Open, line: &str) -> bool {
    let t = line.trim();
    match open {
        Open::Callout(_) => t.len() >= 3 && t.chars().all(|c| c == ':'),
        Open::Details { .. } => t.eq_ignore_ascii_case("</details>"),
    }
}

fn fence_of(line: &str) -> Option<char> {
    let t = line.trim_start();
    if t.starts_with("```") {
        Some('`')
    } else if t.starts_with("~~~") {
        Some('~')
    } else {
        None
    }
}

/// Index of the line closing the container opened at `at`, nested containers of the same kind skipped.
fn find_close(lines: &[&str], at: usize, open: &Open) -> Option<usize> {
    let same = |o: &Open| {
        matches!(
            (o, open),
            (Open::Callout(_), Open::Callout(_)) | (Open::Details { .. }, Open::Details { .. })
        )
    };
    let mut depth = 1;
    let mut fence: Option<char> = None;
    for (i, line) in lines.iter().enumerate().skip(at + 1) {
        if let Some(c) = fence {
            if fence_of(line) == Some(c) {
                fence = None;
            }
            continue;
        }
        if let Some(c) = fence_of(line) {
            fence = Some(c);
        } else if is_close(open, line) {
            depth -= 1;
            if depth == 0 {
                return Some(i);
            }
        } else if opener(line).is_some_and(|o| same(&o)) {
            depth += 1;
        }
    }
    None
}

fn markdown(lines: &[&str], refs: &[&RefSpan]) -> Vec<Node> {
    if lines.iter().all(|l| l.trim().is_empty()) {
        return Vec::new();
    }
    let text = lines.join("\n");
    let mut it = Parser::new_ext(&text, opts()).peekable();
    parse_blocks(&mut it, refs, None)
}

fn parse_lines(lines: &[&str], refs: &[&RefSpan], depth: usize) -> Vec<Node> {
    if depth >= MAX_DEPTH {
        return markdown(lines, refs);
    }
    let mut out = Vec::new();
    let (mut start, mut i) = (0, 0);
    let mut fence: Option<char> = None;
    while i < lines.len() {
        if let Some(c) = fence {
            if fence_of(lines[i]) == Some(c) {
                fence = None;
            }
        } else if let Some(c) = fence_of(lines[i]) {
            fence = Some(c);
        } else if let Some(open) = opener(lines[i]) {
            if let Some(close) = find_close(lines, i, &open) {
                out.extend(markdown(&lines[start..i], refs));
                out.push(container(open, &lines[i + 1..close], refs, depth));
                i = close + 1;
                start = i;
                continue;
            }
        }
        i += 1;
    }
    out.extend(markdown(&lines[start..], refs));
    out
}

fn container(open: Open, body: &[&str], refs: &[&RefSpan], depth: usize) -> Node {
    match open {
        Open::Callout(kind) => Node::Callout {
            kind,
            children: parse_lines(body, refs, depth + 1),
        },
        Open::Details { open, rest } => {
            let mut body: Vec<&str> = std::iter::once(rest.as_str())
                .chain(body.iter().copied())
                .collect();
            let mut summary = Vec::new();
            if let Some(first) = body.iter().position(|l| !l.trim().is_empty()) {
                let t = body[first].trim_start();
                let low = t.to_ascii_lowercase();
                if low.starts_with("<summary>") {
                    if let Some(end) = low.find("</summary>") {
                        let title = &t["<summary>".len()..end];
                        if let Some(Node::Paragraph { children }) =
                            markdown(&[title], refs).into_iter().next()
                        {
                            summary = children;
                        }
                        body.splice(..=first, [&t[end + "</summary>".len()..]]);
                    }
                }
            }
            Node::Details {
                summary,
                open,
                children: parse_lines(&body, refs, depth + 1),
            }
        }
    }
}

type Events<'a> = Peekable<Parser<'a>>;

fn is_inline_start(e: &Event) -> bool {
    matches!(
        e,
        Event::Text(_)
            | Event::FootnoteReference(_)
            | Event::Code(_)
            | Event::SoftBreak
            | Event::HardBreak
            | Event::InlineHtml(_)
            | Event::Start(
                Tag::Emphasis
                    | Tag::Strong
                    | Tag::Strikethrough
                    | Tag::Link { .. }
                    | Tag::Image { .. }
            )
    )
}

fn flush(pending: &mut Vec<Inline>, out: &mut Vec<Node>) {
    if !pending.is_empty() {
        out.push(Node::Paragraph {
            children: std::mem::take(pending),
        });
    }
}

fn parse_blocks(it: &mut Events, refs: &[&RefSpan], until: Option<TagEnd>) -> Vec<Node> {
    let mut out = Vec::new();
    let mut pending: Vec<Inline> = Vec::new();
    while let Some(e) = it.peek() {
        if let (Event::End(t), Some(u)) = (e, &until) {
            if t == u {
                it.next();
                break;
            }
        }
        if is_inline_start(e) {
            parse_inline_item(it, refs, &mut pending);
            continue;
        }
        let Some(ev) = it.next() else { break };
        match ev {
            Event::Start(Tag::Paragraph) => {
                flush(&mut pending, &mut out);
                let children = parse_inlines(it, refs, TagEnd::Paragraph);
                out.push(Node::Paragraph { children });
            }
            Event::Start(Tag::Heading { level, .. }) => {
                flush(&mut pending, &mut out);
                let children = parse_inlines(it, refs, TagEnd::Heading(level));
                out.push(Node::Heading {
                    level: level as u8,
                    children,
                });
            }
            Event::Start(Tag::CodeBlock(kind)) => {
                flush(&mut pending, &mut out);
                let lang = match kind {
                    CodeBlockKind::Fenced(info) => info
                        .split_whitespace()
                        .next()
                        .map(str::to_string)
                        .filter(|s| !s.is_empty()),
                    CodeBlockKind::Indented => None,
                };
                let mut code = String::new();
                for ev in it.by_ref() {
                    match ev {
                        Event::Text(t) => code.push_str(&t),
                        Event::End(TagEnd::CodeBlock) => break,
                        _ => {}
                    }
                }
                if lang.as_deref() == Some("query") {
                    out.push(Node::Query {
                        text: code.trim_end_matches('\n').to_string(),
                    });
                } else {
                    let code = code.strip_suffix('\n').map(str::to_string).unwrap_or(code);
                    out.push(Node::CodeBlock { lang, code });
                }
            }
            Event::Start(Tag::BlockQuote(_)) => {
                flush(&mut pending, &mut out);
                let children = parse_blocks(it, refs, Some(TagEnd::BlockQuote(None)));
                out.push(Node::Quote { children });
            }
            Event::Start(Tag::FootnoteDefinition(label)) => {
                flush(&mut pending, &mut out);
                let children = parse_blocks(it, refs, Some(TagEnd::FootnoteDefinition));
                out.push(Node::Footnote {
                    label: label.to_string(),
                    children,
                });
            }
            Event::Start(Tag::List(first)) => {
                flush(&mut pending, &mut out);
                let mut items = Vec::new();
                while let Some(e) = it.next() {
                    match e {
                        Event::Start(Tag::Item) => {
                            items.push(parse_blocks(it, refs, Some(TagEnd::Item)))
                        }
                        Event::End(TagEnd::List(_)) => break,
                        _ => {}
                    }
                }
                out.push(Node::List {
                    ordered: first.is_some(),
                    items,
                });
            }
            Event::Start(Tag::Table(aligns)) => {
                flush(&mut pending, &mut out);
                out.push(parse_table(it, refs, &aligns));
            }
            Event::Start(Tag::HtmlBlock) => {
                flush(&mut pending, &mut out);
                let mut raw = String::new();
                for ev in it.by_ref() {
                    match ev {
                        Event::Html(t) | Event::Text(t) => raw.push_str(&t),
                        Event::End(TagEnd::HtmlBlock) => break,
                        _ => {}
                    }
                }
                let raw = raw.trim_end_matches('\n').to_string();
                out.push(Node::Paragraph {
                    children: vec![Inline::Text { text: raw }],
                });
            }
            Event::Html(t) => pending.push(Inline::Text {
                text: t.to_string(),
            }),
            Event::Rule => {
                flush(&mut pending, &mut out);
                out.push(Node::Rule);
            }
            Event::End(_) => {
                // unexpected end: leave the loop so the caller can continue
                break;
            }
            _ => {}
        }
    }
    flush(&mut pending, &mut out);
    out
}

fn parse_table(it: &mut Events, refs: &[&RefSpan], aligns: &[Alignment]) -> Node {
    let mut header: Vec<Vec<Inline>> = Vec::new();
    let mut rows: Vec<Vec<Vec<Inline>>> = Vec::new();
    while let Some(e) = it.next() {
        match e {
            Event::Start(Tag::TableHead) => {
                while let Some(e) = it.next() {
                    match e {
                        Event::Start(Tag::TableCell) => {
                            header.push(parse_inlines(it, refs, TagEnd::TableCell))
                        }
                        Event::End(TagEnd::TableHead) => break,
                        _ => {}
                    }
                }
            }
            Event::Start(Tag::TableRow) => {
                let mut row = Vec::new();
                while let Some(e) = it.next() {
                    match e {
                        Event::Start(Tag::TableCell) => {
                            row.push(parse_inlines(it, refs, TagEnd::TableCell))
                        }
                        Event::End(TagEnd::TableRow) => break,
                        _ => {}
                    }
                }
                rows.push(row);
            }
            Event::End(TagEnd::Table) => break,
            _ => {}
        }
    }
    let aligns = aligns
        .iter()
        .map(|a| match a {
            Alignment::Left => "left",
            Alignment::Center => "center",
            Alignment::Right => "right",
            Alignment::None => "none",
        })
        .map(str::to_string)
        .collect();
    Node::Table {
        aligns,
        header,
        rows,
    }
}

fn parse_inlines(it: &mut Events, refs: &[&RefSpan], until: TagEnd) -> Vec<Inline> {
    let mut out = Vec::new();
    while let Some(e) = it.peek() {
        if let Event::End(t) = e {
            if *t == until {
                it.next();
                break;
            }
            // a mismatched end belongs to an outer construct
            break;
        }
        parse_inline_item(it, refs, &mut out);
    }
    out
}

fn allowed_href(h: &str) -> bool {
    let l = h.trim().to_ascii_lowercase();
    l.starts_with("http://")
        || l.starts_with("https://")
        || l.starts_with("mailto:")
        || l.starts_with("file:///")
}

/// A link to a file copied into the graph (`../assets/2026/10/a.pdf`); it is opened with the system's program for it.
fn is_asset_link(h: &str) -> bool {
    let h = h.trim();
    if has_scheme(h) {
        return false;
    }
    let clean = h.trim_start_matches("../").trim_start_matches("./");
    clean.starts_with("assets/") && !clean.contains("/../")
}

fn has_scheme(s: &str) -> bool {
    let head = s.split(['/', '?', '#']).next().unwrap_or("");
    head.contains(':')
}

fn parse_inline_item(it: &mut Events, refs: &[&RefSpan], out: &mut Vec<Inline>) {
    let Some(ev) = it.next() else { return };
    match ev {
        Event::Text(t) => expand_text(&t, refs, out),
        Event::Code(c) => out.push(Inline::Code {
            text: c.to_string(),
        }),
        Event::FootnoteReference(label) => out.push(Inline::FootnoteRef {
            label: label.to_string(),
        }),
        Event::SoftBreak => out.push(Inline::SoftBreak),
        Event::HardBreak => out.push(Inline::HardBreak),
        Event::InlineHtml(h) | Event::Html(h) => out.push(Inline::Text {
            text: h.to_string(),
        }),
        Event::Start(Tag::Emphasis) => out.push(Inline::Italic {
            children: parse_inlines(it, refs, TagEnd::Emphasis),
        }),
        Event::Start(Tag::Strong) => out.push(Inline::Bold {
            children: parse_inlines(it, refs, TagEnd::Strong),
        }),
        Event::Start(Tag::Strikethrough) => out.push(Inline::Strike {
            children: parse_inlines(it, refs, TagEnd::Strikethrough),
        }),
        Event::Start(Tag::Link { dest_url, .. }) => {
            let children = parse_inlines(it, refs, TagEnd::Link);
            if allowed_href(&dest_url) {
                out.push(Inline::Link {
                    href: dest_url.to_string(),
                    children,
                });
            } else if let Some(href) = local_target(&dest_url, false) {
                // a file in the graph (a copied attachment, Nextcloud's `.attachments.N/…`): opened by the shell
                out.push(Inline::Link { href, children });
            } else {
                out.extend(children);
            }
        }
        Event::Start(Tag::Image { dest_url, .. }) => {
            let inner = parse_inlines(it, refs, TagEnd::Image);
            let alt = plain_text(&inner);
            let src = dest_url.to_string();
            if !has_scheme(&src) {
                match local_target(&src, true) {
                    Some(path)
                        if BASE_DIR.with(|b| b.borrow().is_none()) || is_image_path(&path) =>
                    {
                        out.push(Inline::Image {
                            src: path,
                            alt,
                            local: true,
                        });
                    }
                    // not a picture (Nextcloud writes every attachment as `![name](…)`): a link to the file
                    Some(path) => out.push(Inline::Link {
                        href: path.clone(),
                        children: vec![Inline::Text {
                            text: if alt.is_empty() {
                                path.rsplit('/').next().unwrap_or("file").to_string()
                            } else {
                                alt
                            },
                        }],
                    }),
                    None => out.push(Inline::Text { text: alt }),
                }
            } else {
                // remote images are not loaded (privacy); show the alt text as a link when the scheme is safe
                if allowed_href(&src) {
                    out.push(Inline::Link {
                        href: src,
                        children: vec![Inline::Text {
                            text: if alt.is_empty() { "image".into() } else { alt },
                        }],
                    });
                } else {
                    out.push(Inline::Text { text: alt });
                }
            }
        }
        _ => {}
    }
}

fn plain_text(items: &[Inline]) -> String {
    let mut s = String::new();
    for i in items {
        match i {
            Inline::Text { text } | Inline::Code { text } => s.push_str(text),
            Inline::Bold { children }
            | Inline::Italic { children }
            | Inline::Strike { children }
            | Inline::Highlight { children }
            | Inline::Link { children, .. } => {
                s.push_str(&plain_text(children));
            }
            Inline::SoftBreak | Inline::HardBreak => s.push(' '),
            _ => {}
        }
    }
    s
}

/// Replaces placeholder characters by refs and splits `==highlight==` out of plain runs.
fn expand_text(t: &str, refs: &[&RefSpan], out: &mut Vec<Inline>) {
    let mut run = String::new();
    for c in t.chars() {
        let cp = c as u32;
        if (PUA_BASE..PUA_BASE + refs.len() as u32).contains(&cp) {
            push_run(&mut run, out);
            if let Some(r) = refs.get((cp - PUA_BASE) as usize) {
                out.push(ref_inline(r));
            }
        } else {
            run.push(c);
        }
    }
    push_run(&mut run, out);
}

fn ref_inline(r: &RefSpan) -> Inline {
    match r.kind {
        RefKind::Page => Inline::PageRef {
            target: r.target.clone(),
            label: r.label.as_deref().map(|l| {
                let nodes = render(l, &[]);
                match nodes.into_iter().next() {
                    Some(Node::Paragraph { children }) => children,
                    _ => vec![Inline::Text {
                        text: l.to_string(),
                    }],
                }
            }),
        },
        RefKind::Tag => Inline::Tag {
            target: r.target.clone(),
        },
        RefKind::Block => Inline::BlockRef {
            uid: r.target.clone(),
        },
        RefKind::BlockEmbed => Inline::BlockEmbed {
            uid: r.target.clone(),
        },
        RefKind::PageEmbed => Inline::PageEmbed {
            target: r.target.clone(),
        },
    }
}

fn push_run(run: &mut String, out: &mut Vec<Inline>) {
    if run.is_empty() {
        return;
    }
    let s = std::mem::take(run);
    let mut rest = s.as_str();
    while let Some(open) = rest.find("==") {
        let inner_start = open + 2;
        let Some(rel) = rest.get(inner_start..).and_then(|r| r.find("==")) else {
            break;
        };
        let inner = rest.get(inner_start..inner_start + rel).unwrap_or("");
        let ok = !inner.is_empty()
            && !inner.starts_with(char::is_whitespace)
            && !inner.ends_with(char::is_whitespace);
        if !ok {
            // not a highlight: emit up to and including the first `==`, continue after it
            out.push(Inline::Text {
                text: rest.get(..inner_start).unwrap_or("").to_string(),
            });
            rest = rest.get(inner_start..).unwrap_or("");
            continue;
        }
        if open > 0 {
            out.push(Inline::Text {
                text: rest.get(..open).unwrap_or("").to_string(),
            });
        }
        out.push(Inline::Highlight {
            children: vec![Inline::Text {
                text: inner.to_string(),
            }],
        });
        rest = rest.get(inner_start + rel + 2..).unwrap_or("");
    }
    if !rest.is_empty() {
        out.push(Inline::Text {
            text: rest.to_string(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(t: &str) -> Inline {
        Inline::Text { text: t.into() }
    }

    fn render_with_core(src: &str) -> Vec<Node> {
        let (items, _) = notatnik_core::inline::scan(src);
        let refs: Vec<RefSpan> = items
            .into_iter()
            .filter_map(|i| match i {
                notatnik_core::inline::InlineItem::Ref(r) => Some(RefSpan {
                    start: r.span.start,
                    end: r.span.end,
                    kind: r.kind,
                    target: r.target,
                    label: r.label_span.map(|l| l.slice(src).to_string()),
                }),
                _ => None,
            })
            .collect();
        render(src, &refs)
    }

    fn para(children: Vec<Inline>) -> Node {
        Node::Paragraph { children }
    }

    #[test]
    fn emphasis_code_strike_and_highlight() {
        let n = render_with_core("a **b** *c* ~~d~~ `e` ==f== g");
        assert_eq!(
            n,
            vec![para(vec![
                text("a "),
                Inline::Bold {
                    children: vec![text("b")]
                },
                text(" "),
                Inline::Italic {
                    children: vec![text("c")]
                },
                text(" "),
                Inline::Strike {
                    children: vec![text("d")]
                },
                text(" "),
                Inline::Code { text: "e".into() },
                text(" "),
                Inline::Highlight {
                    children: vec![text("f")]
                },
                text(" g"),
            ])]
        );
    }

    #[test]
    fn refs_inside_formatting_and_with_special_names() {
        let n = render_with_core(
            "**see [[snake_case_name]]** and *see #tag_x*: ((abc123)) !((def456)) ![[page]] [[a|my **label**]]",
        );
        let Node::Paragraph { children } = &n[0] else {
            panic!("{n:?}")
        };
        let Inline::Bold { children: bold } = &children[0] else {
            panic!("{children:?}")
        };
        assert_eq!(
            bold[1],
            Inline::PageRef {
                target: "snake_case_name".into(),
                label: None
            }
        );
        assert!(children.contains(&Inline::BlockRef {
            uid: "abc123".into()
        }));
        assert!(children.contains(&Inline::BlockEmbed {
            uid: "def456".into()
        }));
        assert!(children.contains(&Inline::PageEmbed {
            target: "page".into()
        }));
        assert!(
            matches!(children.last(), Some(Inline::PageRef { label: Some(l), .. }) if l.len() == 2)
        );
        let Inline::Italic { children: it } = &children[2] else {
            panic!("{children:?}")
        };
        assert_eq!(
            it[1],
            Inline::Tag {
                target: "tag_x".into()
            }
        );
    }

    #[test]
    fn raw_html_is_literal_text() {
        let n = render_with_core(
            "<script>alert(1)</script> and <b>bold</b> and <img src=x onerror=alert(1)>",
        );
        let all = serde_json::to_string(&n).unwrap();
        assert!(!all.contains("\"type\":\"html\""));
        let Node::Paragraph { children } = &n[0] else {
            panic!("{n:?}")
        };
        let joined: String = children
            .iter()
            .map(|c| {
                if let Inline::Text { text } = c {
                    text.clone()
                } else {
                    "?".into()
                }
            })
            .collect();
        assert!(joined.contains("<script>alert(1)</script>"), "{joined}");
        assert!(joined.contains("<img src=x onerror=alert(1)>"), "{joined}");
        // html block form
        let n = render_with_core("<div onclick=\"x()\">hello</div>\n\nafter");
        let s = serde_json::to_string(&n).unwrap();
        assert!(s.contains("<div onclick=\\\"x()\\\">hello</div>"), "{s}");
    }

    #[test]
    fn only_safe_link_schemes() {
        let n = render_with_core(
            "[ok](https://example.com/a#b) [mail](mailto:a@b.c) [bad](javascript:alert(1)) [rel](other.md) [esc](../assets/../../etc/passwd)",
        );
        let Node::Paragraph { children } = &n[0] else {
            panic!()
        };
        let links: Vec<&str> = children
            .iter()
            .filter_map(|c| {
                if let Inline::Link { href, .. } = c {
                    Some(href.as_str())
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(links, ["https://example.com/a#b", "mailto:a@b.c"]);
        assert!(
            children.contains(&text("bad")),
            "unsafe links keep their text only"
        );
    }

    #[test]
    fn links_to_local_files_and_copied_attachments_are_kept() {
        let n = render_with_core(
            "[a](file:///home/me/My%20Docs/a.pdf) [b](../../assets/2026/10/b-1234abcd.pdf) [c](assets/c.txt) [d](../notes/d.md) [e](../assets/../../etc/passwd)",
        );
        let Node::Paragraph { children } = &n[0] else {
            panic!()
        };
        let links: Vec<&str> = children
            .iter()
            .filter_map(|c| {
                if let Inline::Link { href, .. } = c {
                    Some(href.as_str())
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(
            links,
            [
                "file:///home/me/My%20Docs/a.pdf",
                "../../assets/2026/10/b-1234abcd.pdf",
                "assets/c.txt"
            ]
        );
    }

    #[test]
    fn relative_links_and_images_are_resolved_against_the_pages_folder() {
        let _g = with_base_dir("pages/work");
        let n = render_with_core(
            "![Makefile](.attachments.492/Makefile) ![pic](.attachments.1/a%20b.png) ![cat](../../assets/cat.png) [doc](../../assets/a.pdf) [page](other.md) ![out](../../../x.png) [up](../shared/f.txt)",
        );
        let Node::Paragraph { children } = &n[0] else {
            panic!()
        };
        let shown: Vec<String> = children
            .iter()
            .filter_map(|c| match c {
                Inline::Image { src, .. } => Some(format!("image {src}")),
                Inline::Link { href, .. } => Some(format!("link {href}")),
                _ => None,
            })
            .collect();
        assert_eq!(
            shown,
            [
                // Nextcloud writes every attachment as an image: a file that is not one becomes a link
                "link pages/work/.attachments.492/Makefile",
                "image pages/work/.attachments.1/a b.png",
                "image assets/cat.png",
                "link assets/a.pdf",
                "link pages/shared/f.txt",
            ]
        );
        // a link to a page is not a file; a path out of the graph shows its text only
        assert!(children.contains(&text("page")), "{children:?}");
        assert!(children.contains(&text("out")), "{children:?}");
    }

    #[test]
    fn images_local_only() {
        let n = render_with_core(
            "![cat](../assets/cat.png) ![remote](https://x.y/t.png) ![bad](javascript:1)",
        );
        let Node::Paragraph { children } = &n[0] else {
            panic!()
        };
        assert_eq!(
            children[0],
            Inline::Image {
                src: "../assets/cat.png".into(),
                alt: "cat".into(),
                local: true
            }
        );
        assert!(matches!(&children[2], Inline::Link { href, .. } if href == "https://x.y/t.png"));
        assert!(
            !children
                .iter()
                .any(|c| matches!(c, Inline::Image { src, .. } if src.contains("http")))
        );
    }

    #[test]
    fn blocks_code_table_heading_query() {
        let src = "## Title\n\n```rust\nfn main() {}\n```\n\n| a | b |\n|---|:-:|\n| 1 | [[x]] |\n\n```query\ntask: todo\n```\n\n> quoted";
        let n = render_with_core(src);
        assert_eq!(
            n[0],
            Node::Heading {
                level: 2,
                children: vec![text("Title")]
            }
        );
        assert_eq!(
            n[1],
            Node::CodeBlock {
                lang: Some("rust".into()),
                code: "fn main() {}".into()
            }
        );
        let Node::Table {
            aligns,
            header,
            rows,
        } = &n[2]
        else {
            panic!("{n:?}")
        };
        assert_eq!(aligns, &["none", "center"]);
        assert_eq!(header.len(), 2);
        assert_eq!(
            rows[0][1],
            vec![Inline::PageRef {
                target: "x".into(),
                label: None
            }]
        );
        assert_eq!(
            n[3],
            Node::Query {
                text: "task: todo".into()
            }
        );
        assert!(matches!(&n[4], Node::Quote { .. }));
    }

    #[test]
    fn line_breaks_and_lists() {
        let n = render_with_core("line one\nline two");
        assert_eq!(
            n,
            vec![para(vec![
                text("line one"),
                Inline::SoftBreak,
                text("line two")
            ])]
        );
        let n = render_with_core("1. a\n2. b");
        assert!(matches!(&n[0], Node::List { ordered: true, items } if items.len() == 2));
    }

    #[test]
    fn highlight_edge_cases() {
        let n = render_with_core("a == b == c and ==x==y");
        let Node::Paragraph { children } = &n[0] else {
            panic!()
        };
        let hl: Vec<_> = children
            .iter()
            .filter(|c| matches!(c, Inline::Highlight { .. }))
            .collect();
        assert_eq!(hl.len(), 1);
    }

    #[test]
    fn callouts_and_details_wrap_what_is_inside_and_footnotes_are_kept() {
        let nodes = render_with_core("::: info\nCareful!\n\n:::\n\nafter");
        assert!(
            matches!(&nodes[0], Node::Callout { kind, children } if kind == "info" && children.len() == 1)
        );
        assert_eq!(nodes.len(), 2);

        let src =
            "<details>\n<summary>More *here*</summary>\nHidden\n\n</details>\n::: warning\nx\n:::";
        let nodes = render_with_core(src);
        let Node::Details {
            summary,
            open,
            children,
        } = &nodes[0]
        else {
            panic!("{nodes:?}")
        };
        assert!(!open && matches!(summary[0], Inline::Text { .. }) && summary.len() == 2);
        assert_eq!(children, &vec![para(vec![text("Hidden")])]);
        assert!(matches!(&nodes[1], Node::Callout { kind, .. } if kind == "warning"));

        // no closer, or inside a code fence: ordinary text
        assert!(matches!(
            &render_with_core("::: info\nx")[0],
            Node::Paragraph { .. }
        ));
        assert!(matches!(
            &render_with_core("```\n::: info\nx\n:::\n```")[0],
            Node::CodeBlock { .. }
        ));

        let nodes = render_with_core("a[^1]\n\n[^1]: note");
        assert!(
            matches!(&nodes[0], Node::Paragraph { children } if children.iter().any(|c| matches!(c, Inline::FootnoteRef { label } if label == "1")))
        );
        assert!(matches!(&nodes[1], Node::Footnote { label, .. } if label == "1"));
    }
}
