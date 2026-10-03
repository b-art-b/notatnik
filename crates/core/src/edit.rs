//! Edit operations: [`BlockOp`]s are turned into minimal text patches on the original source using
//! spans, so bytes outside the touched blocks never change (ST-3) and every edit is exactly
//! reversible (ED-6).

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::{Block, Page, Span, TaskState, ids::is_valid_block_id, props::is_valid_key};

/// Addresses a block by its child-index path, optionally guarded by the content hash the caller saw.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockRef {
    /// Child indices from the page root, e.g. `[2, 0]` = first child of the third top-level block.
    pub path: Vec<usize>,
    /// Expected [`Block::content_hash`]; a mismatch rejects the op as stale.
    #[serde(default)]
    pub hash: Option<String>,
}

impl BlockRef {
    /// A reference without a staleness guard.
    pub fn at(path: &[usize]) -> Self {
        BlockRef {
            path: path.to_vec(),
            hash: None,
        }
    }
}

/// One edit to a page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum BlockOp {
    /// Inserts a new block as child `index` of `parent` (empty path = page root).
    Insert {
        /// Parent path.
        parent: Vec<usize>,
        /// Position among the parent's children (`len` appends).
        index: usize,
        /// Block text: first line, then continuation lines, `\n` separated, no bullet or indentation.
        text: String,
    },
    /// Replaces the block's own text (first line, properties, continuation lines). The block ID is
    /// kept. An empty path addresses the preamble.
    UpdateText {
        /// Target.
        block: BlockRef,
        /// New text, as returned by [`Block::edit_text`].
        text: String,
    },
    /// Deletes the block and its children.
    Delete {
        /// Target.
        block: BlockRef,
    },
    /// Makes the block the last child of its previous sibling.
    Indent {
        /// Target.
        block: BlockRef,
    },
    /// Moves the block up one level, in place: it follows its parent, and its later siblings
    /// become its children.
    Outdent {
        /// Target.
        block: BlockRef,
    },
    /// Moves the block (with children). `new_parent` is a path in the tree *before* the move;
    /// `index` counts among the new parent's children *after* the block has been removed.
    Move {
        /// Target.
        block: BlockRef,
        /// Destination parent path.
        new_parent: Vec<usize>,
        /// Destination index.
        index: usize,
    },
    /// Sets or adds a `key:: value` property.
    SetProperty {
        /// Target.
        block: BlockRef,
        /// Property key.
        key: String,
        /// Raw value text.
        value: String,
    },
    /// Removes a property line (no-op if absent).
    RemoveProperty {
        /// Target.
        block: BlockRef,
        /// Property key.
        key: String,
    },
    /// Sets (`Some`) or clears (`None`) the task marker.
    SetTaskState {
        /// Target.
        block: BlockRef,
        /// New state.
        state: Option<TaskState>,
    },
    /// Adds ` ^id` to the first line (no-op if the block already has an ID).
    EnsureBlockId {
        /// Target.
        block: BlockRef,
        /// The ID to use (caller generates it, checked against the index).
        id: String,
    },
    /// Sets a frontmatter key (value is raw YAML, single line), creating the frontmatter if needed.
    SetPageProperty {
        /// Key.
        key: String,
        /// Raw YAML value.
        value: String,
    },
    /// Removes a frontmatter key (no-op if absent).
    RemovePageProperty {
        /// Key.
        key: String,
    },
}

/// A replacement of `range` (in the source the patch applies to) by `new_text`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextPatch {
    /// Range in the source this patch applies to.
    pub range: Span,
    /// The text that was there (kept so the patch can be reversed).
    pub old_text: String,
    /// The replacement.
    pub new_text: String,
}

/// Result of applying an op.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EditResult {
    /// The edited source.
    pub new_source: String,
    /// Non-overlapping patches against the original source, in order.
    pub patches: Vec<TextPatch>,
}

/// Why an op was rejected. The source is never changed on error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditError {
    /// The block changed since the caller read it (content hash mismatch).
    Stale,
    /// No block at that path.
    NoSuchBlock(Vec<usize>),
    /// Index out of range.
    InvalidIndex,
    /// Moving a block into itself or its descendants.
    InvalidMove,
    /// Indent without a previous sibling.
    CannotIndent,
    /// Outdent of a top-level block.
    CannotOutdent,
    /// Bad key, value or ID.
    InvalidValue(String),
    /// The frontmatter is invalid YAML, so it can't be edited safely.
    InvalidFrontmatter,
    /// Not supported for this target (e.g. structural ops on the preamble).
    Unsupported(&'static str),
}

impl fmt::Display for EditError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EditError::Stale => f.write_str("the block changed since it was read"),
            EditError::NoSuchBlock(p) => write!(f, "no block at path {p:?}"),
            EditError::InvalidIndex => f.write_str("index out of range"),
            EditError::InvalidMove => f.write_str("cannot move a block into itself"),
            EditError::CannotIndent => f.write_str("no previous sibling to indent under"),
            EditError::CannotOutdent => f.write_str("block is already at the top level"),
            EditError::InvalidValue(m) => write!(f, "invalid value: {m}"),
            EditError::InvalidFrontmatter => {
                f.write_str("frontmatter is not valid YAML; fix it by hand first")
            }
            EditError::Unsupported(m) => write!(f, "unsupported: {m}"),
        }
    }
}

impl std::error::Error for EditError {}

/// Applies patches (against `source`) and returns the new text.
pub fn apply_patches(source: &str, patches: &[TextPatch]) -> String {
    let mut sorted: Vec<&TextPatch> = patches.iter().collect();
    sorted.sort_by_key(|p| (p.range.start, p.range.end));
    let mut out = String::with_capacity(source.len());
    let mut pos = 0;
    for p in sorted {
        out.push_str(source.get(pos..p.range.start).unwrap_or(""));
        out.push_str(&p.new_text);
        pos = p.range.end.max(pos);
    }
    out.push_str(source.get(pos..).unwrap_or(""));
    out
}

/// The patches that undo `patches` when applied to the edited source.
pub fn invert_patches(patches: &[TextPatch]) -> Vec<TextPatch> {
    let mut sorted: Vec<&TextPatch> = patches.iter().collect();
    sorted.sort_by_key(|p| (p.range.start, p.range.end));
    let mut delta: isize = 0;
    let mut out = Vec::new();
    for p in sorted {
        let start = (p.range.start as isize + delta) as usize;
        out.push(TextPatch {
            range: Span::new(start, start + p.new_text.len()),
            old_text: p.new_text.clone(),
            new_text: p.old_text.clone(),
        });
        delta += p.new_text.len() as isize - p.old_text.len() as isize;
    }
    out
}

fn find<'a>(page: &'a Page, path: &[usize]) -> Option<&'a Block> {
    let (first, rest) = path.split_first()?;
    let mut b = page.blocks.get(*first)?;
    for i in rest {
        b = b.children.get(*i)?;
    }
    Some(b)
}

fn resolve<'a>(source: &str, page: &'a Page, r: &BlockRef) -> Result<&'a Block, EditError> {
    let b = find(page, &r.path).ok_or_else(|| EditError::NoSuchBlock(r.path.clone()))?;
    if let Some(h) = &r.hash {
        if *h != b.content_hash(source) {
            return Err(EditError::Stale);
        }
    }
    Ok(b)
}

fn patch(source: &str, range: Span, new_text: impl Into<String>) -> TextPatch {
    TextPatch {
        range,
        old_text: range.slice(source).to_string(),
        new_text: new_text.into(),
    }
}

/// One patch that turns `old` into `new` by replacing only the differing middle.
fn diff_patch(old: &str, new: &str, base: usize) -> Vec<TextPatch> {
    if old == new {
        return Vec::new();
    }
    let mut prefix = 0;
    for ((i, a), b) in old.char_indices().zip(new.chars()) {
        if a != b {
            break;
        }
        prefix = i + a.len_utf8();
    }
    let mut suffix = 0;
    let max = old.len().min(new.len()) - prefix;
    for (a, b) in old.chars().rev().zip(new.chars().rev()) {
        if a != b || suffix + a.len_utf8() > max {
            break;
        }
        suffix += a.len_utf8();
    }
    let range = Span::new(base + prefix, base + old.len() - suffix);
    vec![TextPatch {
        range,
        old_text: old
            .get(prefix..old.len() - suffix)
            .unwrap_or("")
            .to_string(),
        new_text: new
            .get(prefix..new.len() - suffix)
            .unwrap_or("")
            .to_string(),
    }]
}

fn strip_eol(s: &str) -> &str {
    s.strip_suffix("\r\n")
        .or_else(|| s.strip_suffix('\n'))
        .unwrap_or(s)
}

/// End offset (after the line break) of the line that contains `pos`.
fn line_end_incl(source: &str, pos: usize) -> usize {
    match source.get(pos..).and_then(|r| r.find('\n')) {
        Some(i) => pos + i + 1,
        None => source.len(),
    }
}

/// Inserts whole lines (`lines` has no trailing line break) at a line start or at the end of file.
fn insert_lines(source: &str, offset: usize, lines: &str, eol: &str) -> TextPatch {
    let text = if offset == source.len() && !source.is_empty() && !source.ends_with('\n') {
        format!("{eol}{lines}")
    } else {
        format!("{lines}{eol}")
    };
    TextPatch {
        range: Span::new(offset, offset),
        old_text: String::new(),
        new_text: text,
    }
}

/// Deletes whole lines `[start, end)`; at the end of a file without a final newline the
/// preceding line break goes too, so the file still ends without one.
fn delete_lines(source: &str, start: usize, end: usize) -> TextPatch {
    let mut s = start;
    if end == source.len() && !source.ends_with('\n') && s > 0 {
        if let Some(prev) = source.get(..s) {
            if prev.ends_with("\r\n") {
                s -= 2;
            } else if prev.ends_with('\n') {
                s -= 1;
            }
        }
    }
    patch(source, Span::new(s, end), "")
}

fn new_block_lines(text: &str, indent: &str) -> String {
    let mut out = String::new();
    for (i, line) in text.split('\n').enumerate() {
        let line = line.trim_end_matches('\r');
        if i == 0 {
            out.push_str(indent);
            out.push('-');
            if !line.is_empty() {
                out.push(' ');
                out.push_str(line);
            }
        } else {
            out.push('\n');
            if !line.is_empty() {
                out.push_str(indent);
                out.push_str("  ");
                out.push_str(line);
            }
        }
    }
    out
}

fn with_eol(s: &str, eol: &str) -> String {
    if eol == "\n" {
        s.to_string()
    } else {
        s.replace('\n', eol)
    }
}

struct Dest {
    offset: usize,
    indent: String,
}

/// Where a new child `index` of `parent_path` goes. `skip` excludes one block from the sibling list.
fn destination(
    source: &str,
    page: &Page,
    parent_path: &[usize],
    index: usize,
    skip: Option<&[usize]>,
) -> Result<Dest, EditError> {
    let (siblings, parent): (&[Block], Option<&Block>) = if parent_path.is_empty() {
        (&page.blocks, None)
    } else {
        let p =
            find(page, parent_path).ok_or_else(|| EditError::NoSuchBlock(parent_path.to_vec()))?;
        (&p.children, Some(p))
    };
    let skip_idx = skip.and_then(|s| match s.split_last() {
        Some((last, pp)) if pp == parent_path => Some(*last),
        _ => None,
    });
    let sibs: Vec<&Block> = siblings
        .iter()
        .enumerate()
        .filter(|(i, _)| Some(*i) != skip_idx)
        .map(|(_, b)| b)
        .collect();
    if index > sibs.len() {
        return Err(EditError::InvalidIndex);
    }
    if let Some(b) = sibs.get(index) {
        return Ok(Dest {
            offset: b.span.start,
            indent: b.indent_span.slice(source).to_string(),
        });
    }
    if let Some(last) = sibs.last() {
        return Ok(Dest {
            offset: last.span.end,
            indent: last.indent_span.slice(source).to_string(),
        });
    }
    match parent {
        Some(p) => Ok(Dest {
            offset: p.span.end,
            indent: format!(
                "{}{}",
                p.indent_span.slice(source),
                page.style.indent.unit()
            ),
        }),
        None => Ok(Dest {
            offset: source.len(),
            indent: String::new(),
        }),
    }
}

fn is_blank(line: &str) -> bool {
    line.trim().is_empty()
}

fn leading_ws(line: &str) -> usize {
    line.bytes()
        .take_while(|b| *b == b' ' || *b == b'\t')
        .count()
}

fn finish(source: &str, patches: Vec<TextPatch>) -> EditResult {
    EditResult {
        new_source: apply_patches(source, &patches),
        patches,
    }
}

/// Applies `op` to `source` (which `page` was parsed from).
pub fn apply(source: &str, page: &Page, op: &BlockOp) -> Result<EditResult, EditError> {
    let eol = page.style.eol.as_str();
    let patches = match op {
        BlockOp::Insert {
            parent,
            index,
            text,
        } => {
            let d = destination(source, page, parent, *index, None)?;
            let lines = with_eol(&new_block_lines(text, &d.indent), eol);
            vec![insert_lines(source, d.offset, &lines, eol)]
        }
        BlockOp::UpdateText { block, text } => update_text(source, page, block, text, eol)?,
        BlockOp::Delete { block } => {
            let b = resolve(source, page, block)?;
            vec![delete_lines(source, b.span.start, b.span.end)]
        }
        BlockOp::Indent { block } => {
            let b = resolve(source, page, block)?;
            let (idx, parent_path) = block.path.split_last().ok_or(EditError::CannotIndent)?;
            if *idx == 0 {
                return Err(EditError::CannotIndent);
            }
            let mut prev_path = parent_path.to_vec();
            prev_path.push(idx - 1);
            let prev = find(page, &prev_path).ok_or(EditError::CannotIndent)?;
            let unit = match prev.children.first() {
                Some(c) => {
                    let ci = c.indent_span.slice(source);
                    let pi = prev.indent_span.slice(source);
                    ci.strip_prefix(pi)
                        .filter(|u| !u.is_empty())
                        .map_or_else(|| page.style.indent.unit(), str::to_string)
                }
                None => page.style.indent.unit(),
            };
            let mut out = Vec::new();
            let mut pos = b.span.start;
            for line in b.span.slice(source).split_inclusive('\n') {
                if !is_blank(line) {
                    out.push(patch(source, Span::new(pos, pos), unit.clone()));
                }
                pos += line.len();
            }
            out
        }
        BlockOp::Outdent { block } => {
            let b = resolve(source, page, block)?;
            if block.path.len() < 2 {
                return Err(EditError::CannotOutdent);
            }
            let parent = find(page, block.path.get(..block.path.len() - 1).unwrap_or(&[]))
                .ok_or(EditError::CannotOutdent)?;
            let bi = b.indent_span.len();
            let pi = parent.indent_span.len();
            let remove = bi.saturating_sub(pi);
            if remove == 0 {
                return Err(EditError::CannotOutdent);
            }
            let mut out = Vec::new();
            let mut pos = b.span.start;
            for line in b.span.slice(source).split_inclusive('\n') {
                if !is_blank(line) {
                    let n = remove.min(leading_ws(line));
                    if n > 0 {
                        out.push(patch(source, Span::new(pos, pos + n), ""));
                    }
                }
                pos += line.len();
            }
            out
        }
        BlockOp::Move {
            block,
            new_parent,
            index,
        } => {
            let b = resolve(source, page, block)?;
            if new_parent.starts_with(&block.path) {
                return Err(EditError::InvalidMove);
            }
            let d = destination(source, page, new_parent, *index, Some(&block.path))?;
            move_block(source, page, b, &d, eol)
        }
        BlockOp::SetProperty { block, key, value } => {
            let b = resolve(source, page, block)?;
            set_property(source, b, key, value, page, eol)?
        }
        BlockOp::RemoveProperty { block, key } => {
            let b = resolve(source, page, block)?;
            match b.properties.iter().find(|p| p.key == *key) {
                Some(p) => vec![delete_lines(source, p.span.start, p.span.end)],
                None => Vec::new(),
            }
        }
        BlockOp::SetTaskState { block, state } => {
            let b = resolve(source, page, block)?;
            set_task_state(source, b, *state)
        }
        BlockOp::EnsureBlockId { block, id } => {
            let b = resolve(source, page, block)?;
            if !is_valid_block_id(id) {
                return Err(EditError::InvalidValue(format!(
                    "`{id}` is not a valid block id"
                )));
            }
            if b.block_id.is_some() {
                Vec::new()
            } else {
                let at = b.first_line.start + b.first_line.slice(source).trim_end().len();
                vec![patch(source, Span::new(at, at), format!(" ^{id}"))]
            }
        }
        BlockOp::SetPageProperty { key, value } => {
            set_page_property(source, page, key, value, eol)?
        }
        BlockOp::RemovePageProperty { key } => {
            let Some(fm) = &page.frontmatter else {
                return Ok(finish(source, Vec::new()));
            };
            if !fm.valid {
                return Err(EditError::InvalidFrontmatter);
            }
            match fm.entries.iter().find(|e| e.key == *key) {
                Some(e) => vec![delete_lines(source, e.span.start, e.span.end)],
                None => Vec::new(),
            }
        }
    };
    Ok(finish(source, patches))
}

fn update_text(
    source: &str,
    page: &Page,
    r: &BlockRef,
    text: &str,
    eol: &str,
) -> Result<Vec<TextPatch>, EditError> {
    if r.path.is_empty() {
        let pre = page
            .preamble
            .as_ref()
            .ok_or_else(|| EditError::NoSuchBlock(Vec::new()))?;
        let region = Span::new(
            pre.first_line.start,
            pre.own_span.start + strip_eol(pre.own_span.slice(source)).len(),
        );
        let new = with_eol(
            &text
                .split('\n')
                .map(|l| l.trim_end_matches('\r'))
                .collect::<Vec<_>>()
                .join("\n"),
            eol,
        );
        return Ok(diff_patch(region.slice(source), &new, region.start));
    }
    let b = resolve(source, page, r)?;
    let region_end = b.own_span.start + strip_eol(b.own_span.slice(source)).len();
    let region = Span::new(b.first_line.start, region_end);
    let cont_indent = format!("{}  ", b.indent_span.slice(source));
    let mut lines: Vec<String> = Vec::new();
    for (i, line) in text.split('\n').enumerate() {
        let line = line.trim_end_matches('\r');
        if i == 0 {
            let mut l = line.to_string();
            if let Some(id) = &b.block_id {
                let own = l.trim_end().len() > 7 && {
                    let t = l.trim_end();
                    t.get(t.len() - 7..)
                        .is_some_and(|s| s.starts_with('^') && is_valid_block_id(&s[1..]))
                };
                if !own {
                    l = format!("{}{} ^{}", l.trim_end(), "", id.id);
                }
            }
            lines.push(l);
        } else if line.is_empty() {
            lines.push(String::new());
        } else {
            lines.push(format!("{cont_indent}{line}"));
        }
    }
    let new = lines.join(eol);
    Ok(diff_patch(region.slice(source), &new, region.start))
}

fn move_block(source: &str, page: &Page, b: &Block, d: &Dest, eol: &str) -> Vec<TextPatch> {
    let _ = page;
    let had_final_newline = source.is_empty() || source.ends_with('\n');
    let mut src = source.to_string();
    if !had_final_newline {
        src.push_str(eol);
    }
    let (s, e) = (b.span.start, b.span.end.min(src.len()));
    let e = if e == source.len() && !had_final_newline {
        src.len()
    } else {
        e
    };
    let bi = b.indent_span.len();
    let mut moved = String::new();
    for line in src.get(s..e).unwrap_or("").split_inclusive('\n') {
        if is_blank(line) {
            moved.push_str(line);
        } else {
            moved.push_str(&d.indent);
            moved.push_str(line.get(bi.min(leading_ws(line))..).unwrap_or(line));
        }
    }
    let dest = if d.offset >= source.len() {
        src.len()
    } else {
        d.offset
    };
    let new = if dest >= e {
        format!(
            "{}{}{}{}",
            src.get(..s).unwrap_or(""),
            src.get(e..dest).unwrap_or(""),
            moved,
            src.get(dest..).unwrap_or("")
        )
    } else {
        format!(
            "{}{}{}{}",
            src.get(..dest).unwrap_or(""),
            moved,
            src.get(dest..s).unwrap_or(""),
            src.get(e..).unwrap_or("")
        )
    };
    let mut new = new;
    if !had_final_newline {
        if let Some(t) = new.strip_suffix("\r\n").or_else(|| new.strip_suffix('\n')) {
            new = t.to_string();
        }
    }
    diff_patch(source, &new, 0)
}

fn set_property(
    source: &str,
    b: &Block,
    key: &str,
    value: &str,
    page: &Page,
    eol: &str,
) -> Result<Vec<TextPatch>, EditError> {
    if !is_valid_key(key) {
        return Err(EditError::InvalidValue(format!(
            "`{key}` is not a valid property key ([a-z0-9_-]+)"
        )));
    }
    if value.contains('\n') || value.contains('\r') {
        return Err(EditError::InvalidValue(
            "property values are single-line".into(),
        ));
    }
    let value = value.trim();
    if let Some(p) = b.properties.iter().find(|p| p.key == key) {
        let line_end = p.span.start + strip_eol(p.span.slice(source)).len();
        let from = p.key_span.end + 2;
        let new = if value.is_empty() {
            String::new()
        } else {
            format!(" {value}")
        };
        return Ok(diff_patch(
            source.get(from..line_end).unwrap_or(""),
            &new,
            from,
        ));
    }
    let indent = if let Some(first) = b.properties.first() {
        source
            .get(first.span.start..first.key_span.start)
            .unwrap_or("")
            .to_string()
    } else if let Some(rest) = b.rest_span {
        let l = rest.slice(source);
        l.get(..leading_ws(l)).unwrap_or("").to_string()
    } else {
        format!("{}  ", b.indent_span.slice(source))
    };
    let _ = page;
    let offset = match b.props_span {
        Some(s) => s.end,
        None => line_end_incl(source, b.first_line.start),
    };
    let line = if value.is_empty() {
        format!("{indent}{key}::")
    } else {
        format!("{indent}{key}:: {value}")
    };
    Ok(vec![insert_lines(source, offset, &line, eol)])
}

fn set_task_state(source: &str, b: &Block, state: Option<TaskState>) -> Vec<TextPatch> {
    let fl = b.first_line;
    match (&b.task, state) {
        (Some(t), Some(s)) => vec![patch(source, t.marker_span, s.marker())],
        (Some(t), None) => {
            let text = fl.slice(source);
            let after = t.marker_span.end - fl.start;
            let ws = text
                .get(after..)
                .map_or(0, |r| r.len() - r.trim_start_matches([' ', '\t']).len());
            vec![patch(
                source,
                Span::new(t.marker_span.start, t.marker_span.end + ws),
                "",
            )]
        }
        (None, Some(s)) => {
            let lead = if fl.start == b.bullet_span.end {
                " "
            } else {
                ""
            };
            let trail = if fl.is_empty() { "" } else { " " };
            vec![patch(
                source,
                Span::new(fl.start, fl.start),
                format!("{lead}{}{trail}", s.marker()),
            )]
        }
        (None, None) => Vec::new(),
    }
}

fn set_page_property(
    source: &str,
    page: &Page,
    key: &str,
    value: &str,
    eol: &str,
) -> Result<Vec<TextPatch>, EditError> {
    let valid_key = !key.is_empty()
        && key
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
        && key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'));
    if !valid_key {
        return Err(EditError::InvalidValue(format!(
            "`{key}` is not a valid frontmatter key"
        )));
    }
    if value.contains('\n') || value.contains('\r') {
        return Err(EditError::InvalidValue(
            "frontmatter values are single-line".into(),
        ));
    }
    let line = format!("{key}: {}", value.trim());
    match &page.frontmatter {
        None => {
            let at = if page.style.bom { 3 } else { 0 };
            let blank = if source.len() > at { eol } else { "" };
            Ok(vec![patch(
                source,
                Span::new(at, at),
                format!("---{eol}{line}{eol}---{eol}{blank}"),
            )])
        }
        Some(fm) if !fm.valid => Err(EditError::InvalidFrontmatter),
        Some(fm) => {
            if let Some(e) = fm.entries.iter().find(|e| e.key == key) {
                return Ok(vec![patch(source, e.span, format!("{line}{eol}"))]);
            }
            let text = fm.span.slice(source);
            let body = strip_eol(text);
            let close_start = fm.span.start + body.rfind('\n').map_or(0, |i| i + 1);
            Ok(vec![patch(
                source,
                Span::new(close_start, close_start),
                format!("{line}{eol}"),
            )])
        }
    }
}
