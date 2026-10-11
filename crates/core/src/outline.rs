//! Line-based outline parser: bullets, nesting, properties, code fences (decision D15).

use crate::{
    Block, BlockIdRef, BlockKind, Diagnostic, Eol, IndentStyle, Page, ParseOptions, Property,
    PropertyValue, Ref, Severity, Span, Style, frontmatter,
    ids::is_valid_block_id,
    inline::{InlineItem, scan_line},
    props::{coerce_value, infer_value, split_property_line},
    task::{Moment, Priority, Repeat, Task, TaskState},
};

const MAX_DEPTH: usize = 100;
const TAB_WIDTH: usize = 4;

struct Line {
    start: usize,
    content_end: usize,
    end: usize,
}

fn next_line(src: &str, pos: usize) -> Line {
    let rest = src.as_bytes().get(pos..).unwrap_or(&[]);
    match rest.iter().position(|&b| b == b'\n') {
        Some(i) => {
            let end = pos + i + 1;
            let mut ce = pos + i;
            if i > 0 && rest.get(i - 1) == Some(&b'\r') {
                ce -= 1;
            }
            Line {
                start: pos,
                content_end: ce,
                end,
            }
        }
        None => Line {
            start: pos,
            content_end: src.len(),
            end: src.len(),
        },
    }
}

fn indent_width(ws: &str) -> usize {
    let mut w = 0;
    for c in ws.chars() {
        if c == '\t' {
            w = (w / TAB_WIDTH + 1) * TAB_WIDTH;
        } else {
            w += 1;
        }
    }
    w
}

#[derive(Clone, Copy)]
struct Fence {
    ch: u8,
    len: usize,
}

fn fence_open(rest: &str) -> Option<Fence> {
    let b = rest.as_bytes();
    let ch = *b.first()?;
    if ch != b'`' && ch != b'~' {
        return None;
    }
    let len = b.iter().take_while(|&&c| c == ch).count();
    if len < 3 {
        return None;
    }
    if ch == b'`' && rest.get(len..).is_some_and(|r| r.contains('`')) {
        return None;
    }
    Some(Fence { ch, len })
}

fn fence_close(rest: &str, f: Fence) -> bool {
    let t = rest.trim_end();
    let n = t.bytes().take_while(|&c| c == f.ch).count();
    n >= f.len && n == t.len()
}

fn bullet(rest: &str) -> Option<usize> {
    let c = rest.as_bytes().first()?;
    if !matches!(c, b'-' | b'*' | b'+') {
        return None;
    }
    match rest.as_bytes().get(1) {
        None | Some(b' ') | Some(b'\t') => Some(1),
        _ => None,
    }
}

struct Raw {
    kind: BlockKind,
    indent_width: usize,
    start: usize,
    indent_end: usize,
    bullet: Span,
    first_line: Span,
    own_end: usize,
    prop_lines: Vec<(Span, Span, Span)>,
    props_end: usize,
    rest_start: Option<usize>,
    in_props: bool,
    fence: Option<Fence>,
    fence_open_span: Span,
    scan: Vec<Span>,
}

fn is_task_text(first: &str) -> Option<(TaskState, usize)> {
    let word_end = first.find([' ', '\t']).unwrap_or(first.len());
    let state = TaskState::from_marker(first.get(..word_end)?)?;
    Some((state, word_end))
}

fn build_block(src: &str, raw: Raw, opts: &ParseOptions, diags: &mut Vec<Diagnostic>) -> Block {
    let first_text = raw.first_line.slice(src);
    // Block ID
    let trimmed = first_text.trim_end();
    let mut block_id = None;
    if trimmed.len() > 7 {
        let id_start = trimmed.len() - 7;
        if let (Some(tail), Some(before)) = (trimmed.get(id_start..), trimmed.get(..id_start)) {
            if tail.starts_with('^')
                && is_valid_block_id(tail.get(1..).unwrap_or(""))
                && before.ends_with([' ', '\t'])
            {
                let ws = before.len() - before.trim_end().len();
                let s = raw.first_line.start + id_start - ws;
                block_id = Some(BlockIdRef {
                    id: tail.get(1..).unwrap_or("").to_string(),
                    span: Span::new(s, raw.first_line.start + trimmed.len()),
                });
            }
        }
    } else if trimmed.len() == 7
        && trimmed.starts_with('^')
        && is_valid_block_id(trimmed.get(1..).unwrap_or(""))
    {
        block_id = Some(BlockIdRef {
            id: trimmed.get(1..).unwrap_or("").to_string(),
            span: Span::new(raw.first_line.start, raw.first_line.start + 7),
        });
    }

    // Properties
    let mut properties = Vec::new();
    for (line_span, key_span, value_span) in &raw.prop_lines {
        let key = key_span.slice(src).to_string();
        let raw_value = value_span.slice(src).trim().to_string();
        let value = match opts.property_types.get(&key) {
            Some(ty) => coerce_value(&raw_value, *ty).unwrap_or_else(|msg| {
                diags.push(Diagnostic::new(
                    *value_span,
                    Severity::Warning,
                    "property-type",
                    format!("property `{key}`: {msg}"),
                ));
                PropertyValue::Text(raw_value.clone())
            }),
            None => infer_value(&raw_value),
        };
        properties.push(Property {
            key,
            raw: raw_value,
            value,
            span: *line_span,
            key_span: *key_span,
            value_span: *value_span,
        });
    }
    for (i, p) in properties.iter().enumerate() {
        if properties.iter().take(i).any(|q| q.key == p.key) {
            diags.push(Diagnostic::new(
                p.key_span,
                Severity::Warning,
                "duplicate-property",
                format!("property `{}` appears more than once", p.key),
            ));
        }
    }

    // Task
    let task = if raw.kind == BlockKind::Bullet {
        is_task_text(first_text).map(|(state, end)| {
            let marker_span = Span::new(raw.first_line.start, raw.first_line.start + end);
            let get = |k: &str| properties.iter().find(|p| p.key == k);
            let moment = |k: &str, diags: &mut Vec<Diagnostic>| {
                get(k).and_then(|p| match Moment::parse(&p.raw) {
                    Some(m) => Some(m),
                    None => {
                        diags.push(Diagnostic::new(
                            p.value_span,
                            Severity::Warning,
                            "invalid-date",
                            format!("`{k}` is not a valid date: `{}`", p.raw),
                        ));
                        None
                    }
                })
            };
            let scheduled = moment("scheduled", diags);
            let deadline = moment("deadline", diags);
            let priority = get("priority").and_then(|p| {
                let v = Priority::parse_with(&p.raw, &opts.priority_labels);
                if v.is_none() {
                    diags.push(Diagnostic::new(
                        p.value_span,
                        Severity::Warning,
                        "invalid-priority",
                        format!(
                            "priority must be high, medium, low (or A, B, C), not `{}`",
                            p.raw
                        ),
                    ));
                }
                v
            });
            let repeat = get("repeat").and_then(|p| {
                let v = Repeat::parse(&p.raw);
                if v.is_none() {
                    diags.push(Diagnostic::new(
                        p.value_span,
                        Severity::Warning,
                        "invalid-repeat",
                        format!("unknown repeat value `{}`", p.raw),
                    ));
                }
                v
            });
            let done = get("done")
                .and_then(|p| Moment::parse(&p.raw))
                .map(|m| m.date.and_time(m.time.unwrap_or_default()));
            Task {
                state,
                marker_span,
                scheduled,
                deadline,
                priority,
                repeat,
                done,
            }
        })
    } else {
        None
    };

    // Refs
    let mut items = Vec::new();
    for s in &raw.scan {
        scan_line(s.slice(src), s.start, &mut items, diags);
    }
    let refs: Vec<Ref> = items
        .into_iter()
        .filter_map(|i| {
            if let InlineItem::Ref(r) = i {
                Some(r)
            } else {
                None
            }
        })
        .collect();

    let props_span = if raw.prop_lines.is_empty() {
        None
    } else {
        Some(Span::new(
            raw.prop_lines.first().map_or(0, |p| p.0.start),
            raw.props_end,
        ))
    };
    let rest_span = raw
        .rest_start
        .map(|s| Span::new(s, raw.own_end))
        .filter(|s| !s.is_empty());
    if raw.fence.is_some() {
        diags.push(Diagnostic::new(
            raw.fence_open_span,
            Severity::Warning,
            "unclosed-fence",
            "code fence is never closed",
        ));
    }
    Block {
        kind: raw.kind,
        level: 0,
        span: Span::new(raw.start, raw.own_end),
        own_span: Span::new(raw.start, raw.own_end),
        indent_span: Span::new(raw.start, raw.indent_end),
        bullet_span: raw.bullet,
        first_line: raw.first_line,
        props_span,
        rest_span,
        properties,
        block_id,
        task,
        refs,
        children: Vec::new(),
    }
}

/// Parses a page. Never fails and never panics; problems become diagnostics.
pub fn parse_page(src: &str) -> Page {
    parse_page_with(src, &ParseOptions::default())
}

/// Like [`parse_page`] with config-provided options.
pub fn parse_page_with(src: &str, opts: &ParseOptions) -> Page {
    let mut diags: Vec<Diagnostic> = Vec::new();
    let bom = src.starts_with('\u{feff}');
    let start = if bom { 3 } else { 0 };
    let (frontmatter, body_start) = frontmatter::parse(src, start, &mut diags);

    let mut raws: Vec<Raw> = Vec::new();
    let mut parents: Vec<Option<usize>> = Vec::new();
    let mut stack: Vec<usize> = Vec::new(); // indices into raws
    let mut preamble_raw: Option<Raw> = None;
    let mut cur: Option<Raw> = None;
    let mut cur_is_preamble = false;
    let mut eol_seen: Option<Eol> = None;
    let mut indent_style: Option<IndentStyle> = None;
    let mut over_depth = false;

    let mut pos = body_start;
    while pos < src.len() {
        let line = next_line(src, pos);
        pos = line.end;
        if eol_seen.is_none() && line.end > line.content_end {
            eol_seen = Some(if line.end - line.content_end == 2 {
                Eol::Crlf
            } else {
                Eol::Lf
            });
        }
        let text = src.get(line.start..line.content_end).unwrap_or("");
        let rest = text.trim_start_matches([' ', '\t']);
        let ws_len = text.len() - rest.len();
        let ws = text.get(..ws_len).unwrap_or("");
        let width = indent_width(ws);
        let blank = rest.trim().is_empty();

        if let Some(c) = cur.as_mut() {
            if let Some(f) = c.fence {
                let ends = !blank && bullet(rest).is_some() && width <= c.indent_width;
                if !ends {
                    if fence_close(rest, f) {
                        c.fence = None;
                    }
                    if !blank {
                        c.own_end = line.end;
                    }
                    continue;
                }
                // implicit end of an unclosed fence: report it, then treat the line normally
                diags.push(Diagnostic::new(
                    c.fence_open_span,
                    Severity::Warning,
                    "unclosed-fence",
                    "code fence is never closed",
                ));
                c.fence = None;
            }
        }

        if blank {
            if let Some(c) = cur.as_mut() {
                c.in_props = false;
            }
            continue;
        }

        if let Some(bl) = bullet(rest) {
            if let Some(done) = cur.take() {
                if cur_is_preamble {
                    preamble_raw = Some(done);
                } else {
                    raws.push(done);
                }
            }
            cur_is_preamble = false;
            // tree position
            while stack
                .last()
                .is_some_and(|&t| raws.get(t).is_some_and(|r| r.indent_width >= width))
            {
                stack.pop();
            }
            if stack.len() >= MAX_DEPTH {
                stack.pop();
                if !over_depth {
                    over_depth = true;
                    diags.push(Diagnostic::new(
                        Span::new(line.start, line.content_end),
                        Severity::Warning,
                        "too-deep",
                        format!("nesting deeper than {MAX_DEPTH} levels is flattened"),
                    ));
                }
            }
            let parent = stack.last().copied();
            if indent_style.is_none() {
                if let Some(p) = parent.and_then(|p| raws.get(p)) {
                    indent_style = Some(if ws.contains('\t') {
                        IndentStyle::Tabs
                    } else {
                        let d = width.saturating_sub(p.indent_width).clamp(1, 8);
                        IndentStyle::Spaces(d as u8)
                    });
                }
            }
            parents.push(parent);
            stack.push(raws.len());
            let after = rest.get(bl..).unwrap_or("");
            let gap = after.len() - after.trim_start_matches([' ', '\t']).len();
            let fl_start = line.start + ws_len + bl + gap;
            let first_line = Span::new(fl_start.min(line.content_end), line.content_end);
            let mut raw = Raw {
                kind: BlockKind::Bullet,
                indent_width: width,
                start: line.start,
                indent_end: line.start + ws_len,
                bullet: Span::new(line.start + ws_len, line.start + ws_len + bl),
                first_line,
                own_end: line.end,
                prop_lines: Vec::new(),
                props_end: line.end,
                rest_start: None,
                in_props: true,
                fence: None,
                fence_open_span: Span::default(),
                scan: Vec::new(),
            };
            if let Some(f) = fence_open(first_line.slice(src)) {
                raw.fence = Some(f);
                raw.fence_open_span = first_line;
                raw.in_props = false;
            } else {
                raw.scan.push(first_line);
            }
            cur = Some(raw);
            continue;
        }

        // continuation or preamble text
        if cur.is_none() {
            cur_is_preamble = true;
            cur = Some(Raw {
                kind: BlockKind::Preamble,
                indent_width: 0,
                start: line.start + ws_len,
                indent_end: line.start + ws_len,
                bullet: Span::new(line.start + ws_len, line.start + ws_len),
                first_line: Span::new(line.start + ws_len, line.content_end),
                own_end: line.end,
                prop_lines: Vec::new(),
                props_end: line.end,
                rest_start: None,
                in_props: false,
                fence: None,
                fence_open_span: Span::default(),
                scan: Vec::new(),
            });
            if let Some(c) = cur.as_mut() {
                if let Some(f) = fence_open(rest) {
                    c.fence = Some(f);
                    c.fence_open_span = c.first_line;
                } else {
                    c.scan.push(c.first_line);
                }
            }
            continue;
        }
        if let Some(c) = cur.as_mut() {
            c.own_end = line.end;
            if c.in_props {
                if let Some((key, val)) = split_property_line(rest) {
                    let key_start = line.start + ws_len;
                    let key_span = Span::new(key_start, key_start + key.len());
                    let vs = key_start + key.len() + 2;
                    let lead = val.len() - val.trim_start().len();
                    let value_span = Span::new((vs + lead).min(line.content_end), line.content_end);
                    c.prop_lines
                        .push((Span::new(line.start, line.end), key_span, value_span));
                    c.props_end = line.end;
                    c.scan
                        .push(Span::new(line.start + ws_len, line.content_end));
                    continue;
                }
                c.in_props = false;
            }
            if c.rest_start.is_none() {
                c.rest_start = Some(line.start);
            }
            if let Some(f) = fence_open(rest) {
                c.fence = Some(f);
                c.fence_open_span = Span::new(line.start + ws_len, line.content_end);
            } else {
                c.scan
                    .push(Span::new(line.start + ws_len, line.content_end));
            }
        }
    }
    if let Some(done) = cur.take() {
        if cur_is_preamble {
            preamble_raw = Some(done);
        } else {
            raws.push(done);
        }
    }

    // Build blocks
    let n = raws.len();
    let mut blocks: Vec<Option<Block>> = Vec::with_capacity(n);
    for raw in raws {
        blocks.push(Some(build_block(src, raw, opts, &mut diags)));
    }
    let preamble = preamble_raw.map(|r| build_block(src, r, opts, &mut diags));

    // levels
    let mut levels = vec![0usize; n];
    for i in 0..n {
        if let Some(Some(p)) = parents.get(i) {
            levels[i] = levels.get(*p).copied().unwrap_or(0) + 1;
        }
    }
    let mut roots: Vec<Block> = Vec::new();
    for i in (0..n).rev() {
        let Some(mut b) = blocks.get_mut(i).and_then(Option::take) else {
            continue;
        };
        b.children.reverse();
        b.level = levels.get(i).copied().unwrap_or(0);
        if let Some(last) = b.children.last() {
            b.span.end = b.span.end.max(last.span.end);
        }
        match parents.get(i).copied().flatten() {
            Some(p) => {
                if let Some(Some(pb)) = blocks.get_mut(p) {
                    pb.children.push(b);
                }
            }
            None => roots.push(b),
        }
    }
    roots.reverse();

    let style = Style {
        eol: eol_seen.unwrap_or(Eol::Lf),
        indent: indent_style.unwrap_or(IndentStyle::Spaces(2)),
        bom,
        final_newline: src.is_empty() || src.ends_with('\n'),
    };
    let mut seen: Vec<&str> = Vec::new();
    let mut flat: Vec<&Block> = Vec::new();
    for b in &roots {
        flat.push(b);
    }
    let mut i = 0;
    let mut dup_diags = Vec::new();
    while let Some(b) = flat.get(i).copied() {
        i += 1;
        if let Some(id) = &b.block_id {
            if seen.contains(&id.id.as_str()) {
                dup_diags.push(Diagnostic::new(
                    id.span,
                    Severity::Warning,
                    "duplicate-block-id",
                    format!("block id `{}` is used more than once", id.id),
                ));
            } else {
                seen.push(&id.id);
            }
        }
        flat.extend(b.children.iter());
    }
    diags.extend(dup_diags);
    Page {
        style,
        frontmatter,
        preamble,
        blocks: roots,
        diagnostics: diags,
        source_len: src.len(),
    }
}
