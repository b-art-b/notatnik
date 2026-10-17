//! Building [`PageView`]s from parsed pages.

use std::collections::{HashMap, HashSet};

use notatnik_core::{Block, BlockIdentity, BlockKind, Page, PropertyValue};

use crate::dto::{BlockView, FrontmatterView, LogEntryView, Node, PageView, PropView, TaskView};
use crate::render::{RefSpan, render};

/// Per-device and per-graph display settings used while building views.
pub struct ViewOptions<'a> {
    /// Property keys hidden in the rendered view.
    pub hidden: &'a HashSet<String>,
    /// Content hashes of collapsed blocks.
    pub collapsed: &'a HashSet<String>,
    /// Block IDs that some task's `follows::` points to.
    pub followed: &'a HashSet<String>,
}

fn prop_view(key: &str, raw: &str, value: &PropertyValue, hidden: &HashSet<String>) -> PropView {
    let (kind, links) = match value {
        PropertyValue::Text(_) => ("text", Vec::new()),
        PropertyValue::Number(_) => ("number", Vec::new()),
        PropertyValue::Bool(_) => ("bool", Vec::new()),
        PropertyValue::Date(_) => ("date", Vec::new()),
        PropertyValue::DateTime(_) => ("datetime", Vec::new()),
        PropertyValue::Links(l) => ("link", l.clone()),
        PropertyValue::List(_) => ("list", Vec::new()),
    };
    PropView {
        key: key.to_string(),
        raw: raw.to_string(),
        kind: kind.to_string(),
        links,
        hidden: hidden.contains(key),
    }
}

/// The text rendered for a block (first line without task marker and `^id`, continuation lines
/// dedented, property lines skipped) and its refs with offsets into that text.
pub(crate) fn content_text(b: &Block, source: &str) -> (String, Vec<RefSpan>) {
    let mut out = String::new();
    let mut segments: Vec<(usize, usize, usize)> = Vec::new(); // src_start, src_end, out_start
    let own_end = b.own_span.end;
    // text between bullets keeps its indentation (nested lists, aligned code)
    let indent_w = if b.kind == BlockKind::Text {
        0
    } else {
        b.indent_span.len() + 2
    };
    let props = b.props_span;
    let mut pos = b.first_line.start;
    let mut first = true;
    let own: Vec<&str> = source
        .get(b.first_line.start..own_end)
        .unwrap_or("")
        .split_inclusive('\n')
        .collect();
    // the logbook drawer is not rendered as text (it has its own list)
    let drawer = {
        let bodies: Vec<&str> = own
            .iter()
            .map(|l| l.trim_end_matches(['\n', '\r']))
            .collect();
        notatnik_core::logbook::find(&bodies)
    };
    for (idx, line) in own.iter().copied().enumerate() {
        let line_start = pos;
        pos += line.len();
        if idx > 0 && drawer.as_ref().is_some_and(|d| d.contains(&idx)) {
            continue;
        }
        let body = line.trim_end_matches(['\n', '\r']);
        let (mut skip_front, mut end) = (0usize, body.len());
        if first {
            if let Some(t) = &b.task {
                let after = t.marker_span.end - line_start;
                let ws = body
                    .get(after..)
                    .map_or(0, |r| r.len() - r.trim_start_matches([' ', '\t']).len());
                skip_front = after + ws;
            }
            if let Some(id) = &b.block_id {
                end = end.min(id.span.start.saturating_sub(line_start));
            }
        } else {
            if props.is_some_and(|p| line_start >= p.start && line_start < p.end) {
                continue;
            }
            let ws = body
                .bytes()
                .take_while(|c| *c == b' ' || *c == b'\t')
                .count();
            skip_front = ws.min(indent_w);
        }
        let text = body.get(skip_front..end.max(skip_front)).unwrap_or("");
        if !first {
            out.push('\n');
        }
        segments.push((
            line_start + skip_front,
            line_start + skip_front + text.len(),
            out.len(),
        ));
        out.push_str(text);
        first = false;
    }
    let trimmed_len = out.trim_end().len();
    out.truncate(trimmed_len);
    let refs = b
        .refs
        .iter()
        .filter_map(|r| {
            let seg = segments
                .iter()
                .find(|(s, e, _)| r.span.start >= *s && r.span.end <= *e)?;
            let start = seg.2 + (r.span.start - seg.0);
            let end = start + r.span.len();
            (end <= out.len()).then(|| RefSpan {
                start,
                end,
                kind: r.kind,
                target: r.target.clone(),
                label: r.label_span.map(|l| l.slice(source).to_string()),
            })
        })
        .collect();
    (out, refs)
}

/// Renders a short single-line text (a logbook note) with its links, tags and emphasis.
fn render_inline_text(text: &str) -> Vec<Node> {
    if text.trim().is_empty() {
        return Vec::new();
    }
    let mut items = Vec::new();
    notatnik_core::inline::scan_line(text, 0, &mut items, &mut Vec::new());
    let refs: Vec<RefSpan> = items
        .into_iter()
        .filter_map(|i| match i {
            notatnik_core::inline::InlineItem::Ref(r) => Some(RefSpan {
                start: r.span.start,
                end: r.span.end,
                kind: r.kind,
                target: r.target.clone(),
                label: r.label_span.map(|l| l.slice(text).to_string()),
            }),
            notatnik_core::inline::InlineItem::Highlight(_) => None,
        })
        .collect();
    render(text, &refs)
}

fn block_view(
    b: &Block,
    source: &str,
    ids: &HashMap<Vec<usize>, u64>,
    path: &mut Vec<usize>,
    o: &ViewOptions,
) -> BlockView {
    let key: Vec<usize> = if b.kind == BlockKind::Preamble {
        vec![usize::MAX]
    } else {
        path.clone()
    };
    let (text, refs) = content_text(b, source);
    let content: Vec<Node> = if text.trim().is_empty() {
        Vec::new()
    } else {
        render(&text, &refs)
    };
    let task = b.task.as_ref().map(|t| TaskView {
        state: t.state,
        checkbox: t.checkbox,
        scheduled: t.scheduled.map(|m| m.to_string()),
        deadline: t.deadline.map(|m| m.to_string()),
        priority: t.priority.as_ref().map(|p| p.name().to_string()),
        priority_rank: t.priority.as_ref().map(|p| u32::from(p.rank())),
        repeat: b
            .properties
            .iter()
            .find(|p| p.key == "repeat")
            .map(|p| p.raw.clone()),
        // part of a chain of follow-up tasks (TK-15): it follows one, or one follows it
        chain: b.properties.iter().any(|p| p.key == "follows")
            || b.block_id
                .as_ref()
                .is_some_and(|id| o.followed.contains(id.id.as_str())),
    });
    let hash = b.content_hash(source);
    let logbook = {
        let own: Vec<&str> = source
            .get(b.first_line.start..b.own_span.end)
            .unwrap_or("")
            .lines()
            .skip(1)
            .collect();
        notatnik_core::logbook::entries(&own)
            .into_iter()
            .map(|e| LogEntryView {
                when: e.when,
                content: render_inline_text(&e.text),
                text: e.text,
            })
            .collect()
    };
    let mut children = Vec::with_capacity(b.children.len());
    for (i, c) in b.children.iter().enumerate() {
        path.push(i);
        children.push(block_view(c, source, ids, path, o));
        path.pop();
    }
    BlockView {
        id: ids.get(&key).copied().unwrap_or(0) as u32,
        path: if b.kind == BlockKind::Preamble {
            Vec::new()
        } else {
            path.iter().map(|x| *x as u32).collect()
        },
        plain: b.kind == BlockKind::Text,
        collapsed: o.collapsed.contains(&hash),
        hash,
        uid: b.block_id.as_ref().map(|i| i.id.clone()),
        text: b.edit_text(source),
        content,
        task,
        logbook,
        props: b
            .properties
            .iter()
            .map(|p| prop_view(&p.key, &p.raw, &p.value, o.hidden))
            .collect(),
        children,
    }
}

/// Everything needed to describe a page that has a file (or an empty one).
pub struct PageInput<'a> {
    pub name: &'a str,
    pub path: &'a str,
    pub exists: bool,
    pub is_journal: bool,
    pub hash: Option<String>,
    pub source: &'a str,
    pub page: &'a Page,
    pub child_pages: Vec<String>,
}

/// Builds the view. Runtime block IDs come from `tracker` (matching against the previous version).
pub fn page_view(input: PageInput, tracker: &mut BlockIdentity, o: &ViewOptions) -> PageView {
    // relative links and images are resolved against the page's folder while the blocks are rendered
    let dir = input.path.rsplit_once('/').map_or("", |(d, _)| d);
    let _base = crate::render::with_base_dir(dir);
    let tracked = tracker.assign(input.page, input.source);
    let ids: HashMap<Vec<usize>, u64> = tracked.into_iter().map(|t| (t.path, t.id)).collect();
    let preamble = input
        .page
        .preamble
        .as_ref()
        .map(|b| block_view(b, input.source, &ids, &mut Vec::new(), o));
    let mut blocks = Vec::with_capacity(input.page.blocks.len());
    for (i, b) in input.page.blocks.iter().enumerate() {
        blocks.push(block_view(b, input.source, &ids, &mut vec![i], o));
    }
    let fm = input.page.frontmatter.as_ref();
    let frontmatter = fm.map(|f| FrontmatterView {
        valid: f.valid,
        title: f.title.clone(),
        aliases: f.aliases.clone(),
        tags: f.tags.clone(),
        props: f
            .properties
            .iter()
            .map(|p| prop_view(&p.key, &p.raw, &p.value, o.hidden))
            .collect(),
    });
    let title = fm
        .filter(|f| f.valid)
        .and_then(|f| f.title.clone())
        .unwrap_or_else(|| input.name.to_string());
    PageView {
        name: input.name.to_string(),
        title,
        path: input.path.to_string(),
        exists: input.exists,
        is_journal: input.is_journal,
        hash: input.hash,
        frontmatter,
        preamble,
        blocks,
        child_pages: input.child_pages,
        diagnostics: input
            .page
            .diagnostics
            .iter()
            .map(|d| crate::dto::DiagnosticView {
                severity: match d.severity {
                    notatnik_core::Severity::Error => "error",
                    notatnik_core::Severity::Warning => "warning",
                    notatnik_core::Severity::Info => "info",
                }
                .to_string(),
                message: d.message.clone(),
            })
            .collect(),
    }
}

/// Finds a block view by path.
pub fn find_block<'a>(blocks: &'a [BlockView], path: &[u32]) -> Option<&'a BlockView> {
    let (first, rest) = path.split_first()?;
    let mut b = blocks.get(*first as usize)?;
    for i in rest {
        b = b.children.get(*i as usize)?;
    }
    Some(b)
}
