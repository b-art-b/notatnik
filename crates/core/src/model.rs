//! The parsed representation of a page. Everything carries byte [`Span`]s into the original source
//! so edits can be minimal text replacements (ST-3).

use std::collections::BTreeMap;

use chrono::{NaiveDate, NaiveDateTime};
use serde::Serialize;

use crate::{Diagnostic, Span, Task};

/// Line-ending style of a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Eol {
    /// `\n`
    Lf,
    /// `\r\n`
    Crlf,
}

impl Eol {
    /// The line ending as text.
    pub const fn as_str(self) -> &'static str {
        match self {
            Eol::Lf => "\n",
            Eol::Crlf => "\r\n",
        }
    }
}

/// Indentation style of a file, used for lines the app adds or moves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", content = "width", rename_all = "snake_case")]
pub enum IndentStyle {
    /// N spaces per level.
    Spaces(u8),
    /// One tab per level.
    Tabs,
}

impl IndentStyle {
    /// One level of indentation as text.
    pub fn unit(self) -> String {
        match self {
            IndentStyle::Spaces(n) => " ".repeat(usize::from(n)),
            IndentStyle::Tabs => "\t".to_string(),
        }
    }
}

/// Formatting facts about a file that edits must preserve (file-format.md §3.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Style {
    /// Line endings.
    pub eol: Eol,
    /// Indentation of nested blocks.
    pub indent: IndentStyle,
    /// File starts with a UTF-8 BOM.
    pub bom: bool,
    /// File ends with a line break (an empty file counts as yes).
    pub final_newline: bool,
}

impl Default for Style {
    fn default() -> Self {
        Style {
            eol: Eol::Lf,
            indent: IndentStyle::Spaces(2),
            bom: false,
            final_newline: true,
        }
    }
}

/// Typed value of a property.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum PropertyValue {
    /// Plain text.
    Text(String),
    /// Number.
    Number(f64),
    /// `true` / `false`.
    Bool(bool),
    /// `YYYY-MM-DD`.
    Date(NaiveDate),
    /// `YYYY-MM-DD HH:MM`.
    DateTime(NaiveDateTime),
    /// One or more page links (normalised names).
    Links(Vec<String>),
    /// Comma-separated list of scalars of the same type.
    List(Vec<PropertyValue>),
}

/// Declared property type from `config.toml` (`[properties] types`), overriding inference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropType {
    /// Always text.
    Text,
    /// Number.
    Number,
    /// Date.
    Date,
    /// Date with time.
    DateTime,
    /// Boolean.
    Bool,
    /// Page link(s).
    Page,
}

/// Options for parsing; core does no I/O, so config comes in as a parameter.
#[derive(Debug, Clone, Default)]
pub struct ParseOptions {
    /// Declared property types by key.
    pub property_types: BTreeMap<String, PropType>,
    /// The graph's priority labels, most important first (`[tasks] priorities`, `A`, `B`, `C` by default). In a
    /// file `priority::` may be `high`/`medium`/`low`, `A`/`B`/`C` or one of these labels.
    pub priority_labels: Vec<String>,
}

/// A `key:: value` block property, or a frontmatter key.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Property {
    /// Key.
    pub key: String,
    /// Raw value text, trimmed.
    pub raw: String,
    /// Typed value.
    pub value: PropertyValue,
    /// The whole property (all its lines, including line breaks).
    pub span: Span,
    /// The key text.
    pub key_span: Span,
    /// The value text on the first line (empty span if none).
    pub value_span: Span,
}

/// Kind of reference found in text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RefKind {
    /// `[[page]]`
    Page,
    /// `#tag`
    Tag,
    /// `((id))`
    Block,
    /// `!((id))`
    BlockEmbed,
    /// `![[page]]`
    PageEmbed,
}

/// A link, tag, block reference or embed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Ref {
    /// What kind of reference.
    pub kind: RefKind,
    /// Normalised page name, or the block ID for block refs.
    pub target: String,
    /// The whole syntax, e.g. `[[a|label]]`.
    pub span: Span,
    /// Just the target text inside the syntax (what a rename rewrites).
    pub target_span: Span,
    /// The label of `[[a|label]]`.
    pub label_span: Option<Span>,
}

/// A block ID found at the end of the first line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BlockIdRef {
    /// The six-character ID.
    pub id: String,
    /// The ` ^id` text including the whitespace before it.
    pub span: Span,
}

/// Whether a block came from a bullet or is the text before the first bullet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BlockKind {
    /// A `- ` list item.
    Bullet,
    /// Text before the first bullet.
    Preamble,
    /// Unindented text between bullets that follows a blank line (a heading, a table, a code fence, a paragraph,
    /// a footnote ...), as other Markdown editors write it. Kept as written, edited as raw Markdown, never nested.
    Text,
}

/// One block of the outline.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Block {
    /// Bullet or preamble.
    pub kind: BlockKind,
    /// Nesting level (0 = top level), by tree depth, regardless of raw indentation.
    pub level: usize,
    /// From the start of the bullet line (including indentation) to the end of the last descendant.
    pub span: Span,
    /// From the start of the bullet line to the end of the block's own lines, excluding children.
    pub own_span: Span,
    /// Leading whitespace of the bullet line.
    pub indent_span: Span,
    /// The bullet marker (`-`, `*`, `+`); empty for the preamble.
    pub bullet_span: Span,
    /// First line's text after the marker (including task marker and ID), without line break.
    pub first_line: Span,
    /// All property lines (including line breaks), if any.
    pub props_span: Option<Span>,
    /// Continuation lines after the properties, if any.
    pub rest_span: Option<Span>,
    /// Parsed `key:: value` properties.
    pub properties: Vec<Property>,
    /// Block ID, if present.
    pub block_id: Option<BlockIdRef>,
    /// Task data, if the first line starts with a task marker.
    pub task: Option<Task>,
    /// References in the block's own text (not its children).
    pub refs: Vec<Ref>,
    /// Child blocks.
    pub children: Vec<Block>,
}

/// Reserved and user properties of the YAML frontmatter.
#[derive(Debug, Clone, PartialEq, Serialize, Default)]
pub struct Frontmatter {
    /// From the opening `---` to the end of the closing `---` line.
    pub span: Span,
    /// False if the YAML is invalid; the text is preserved and nothing is extracted.
    pub valid: bool,
    /// `title`
    pub title: Option<String>,
    /// `aliases`
    pub aliases: Vec<String>,
    /// `tags`
    pub tags: Vec<String>,
    /// `template`
    pub template: Option<String>,
    /// `icon`
    pub icon: Option<String>,
    /// All other keys.
    pub properties: Vec<Property>,
    /// All top-level keys with their spans (reserved ones included), for edits.
    pub entries: Vec<Property>,
}

/// A parsed page. Spans index into the original source.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Page {
    /// Formatting facts to preserve.
    pub style: Style,
    /// YAML frontmatter, if the file starts with one.
    pub frontmatter: Option<Frontmatter>,
    /// Text before the first bullet.
    pub preamble: Option<Block>,
    /// Top-level blocks.
    pub blocks: Vec<Block>,
    /// Problems found.
    pub diagnostics: Vec<Diagnostic>,
    /// Length of the source in bytes.
    pub source_len: usize,
}

impl Block {
    /// The block's text for the editor: first line (after the bullet) plus continuation lines, with
    /// the block ID and indentation removed and `\n` line breaks.
    pub fn edit_text(&self, source: &str) -> String {
        let start = self.first_line.start;
        let end = self.own_span.end;
        let raw = source.get(start..end).unwrap_or("");
        // text outside the outline is shown exactly as written; only a bullet's lines lose their indentation
        let indent_w = if self.kind == BlockKind::Bullet {
            self.indent_span.len() + 2
        } else {
            0
        };
        let mut out = String::new();
        for (i, line) in raw.split_inclusive('\n').enumerate() {
            let body = line.trim_end_matches(['\n', '\r']);
            let piece = if i == 0 {
                match &self.block_id {
                    Some(id) => {
                        let end = id.span.start.saturating_sub(start);
                        body.get(..end).unwrap_or(body)
                    }
                    None => body,
                }
            } else {
                let ws = body
                    .bytes()
                    .take_while(|b| *b == b' ' || *b == b'\t')
                    .count();
                body.get(ws.min(indent_w)..).unwrap_or(body)
            };
            if i > 0 {
                out.push('\n');
            }
            out.push_str(piece);
        }
        out
    }

    /// Stable hash of the block's own text, used to reject stale edit operations.
    pub fn content_hash(&self, source: &str) -> String {
        let s = source
            .get(self.bullet_span.start.min(self.own_span.end)..self.own_span.end)
            .unwrap_or("");
        crate::hash::content_hash(s.trim_end_matches(['\n', '\r']))
    }

    /// Text for full-text search: the block's own lines without properties and ID, links as their
    /// label (or target) text.
    pub fn search_text(&self, source: &str) -> String {
        let mut out = String::new();
        let mut skip: Vec<Span> = self.properties.iter().map(|p| p.span).collect();
        if let Some(id) = &self.block_id {
            skip.push(id.span);
        }
        let start = self.first_line.start;
        let end = self.own_span.end;
        let mut pos = start;
        let mut first = true;
        let mut first_line_done = false;
        for line in source.get(start..end).unwrap_or("").split_inclusive('\n') {
            let line_span = Span::new(pos, pos + line.len());
            pos += line.len();
            if skip
                .iter()
                .any(|s| s.contains_span(&line_span) && s.start != line_span.end)
                && !first
            {
                continue;
            }
            let body_end = line_span.start + line.trim_end_matches(['\n', '\r']).len();
            let mut cur = line_span.start;
            let mut text = String::new();
            let mut cuts: Vec<(Span, String)> = Vec::new();
            for r in &self.refs {
                if r.span.start >= cur && r.span.end <= body_end {
                    let shown = match r.label_span {
                        Some(l) => l.slice(source).to_string(),
                        None => match r.kind {
                            crate::RefKind::Block | crate::RefKind::BlockEmbed => String::new(),
                            crate::RefKind::Tag => r.target.clone(),
                            _ => r.target.clone(),
                        },
                    };
                    cuts.push((r.span, shown));
                }
            }
            if let Some(id) = &self.block_id {
                if id.span.start >= cur && id.span.end <= body_end {
                    cuts.push((id.span, String::new()));
                }
            }
            cuts.sort_by_key(|(s, _)| s.start);
            for (s, rep) in cuts {
                if s.start < cur {
                    continue;
                }
                text.push_str(source.get(cur..s.start).unwrap_or(""));
                text.push_str(&rep);
                cur = s.end;
            }
            text.push_str(source.get(cur..body_end).unwrap_or(""));
            if !first {
                out.push('\n');
            }
            first = false;
            out.push_str(if first_line_done {
                text.trim()
            } else {
                text.trim_end()
            });
            first_line_done = true;
        }
        out
    }
}

impl Page {
    /// Top-level spans in document order: frontmatter, preamble, then each top-level block.
    pub fn covered_spans(&self) -> Vec<Span> {
        let mut v = Vec::new();
        if let Some(f) = &self.frontmatter {
            v.push(f.span);
        }
        if let Some(p) = &self.preamble {
            v.push(p.span);
        }
        v.extend(self.blocks.iter().map(|b| b.span));
        v
    }

    /// Spans of `source` that are not covered by frontmatter, preamble or blocks and are not blank.
    /// Always empty for a correctly parsed page (P1.6 coverage invariant).
    pub fn uncovered_non_blank(&self, source: &str) -> Vec<Span> {
        let mut out = Vec::new();
        let mut pos = if self.style.bom { 3 } else { 0 };
        for s in self.covered_spans() {
            if s.start > pos && !source.get(pos..s.start).unwrap_or("x").trim().is_empty() {
                out.push(Span::new(pos, s.start));
            }
            pos = pos.max(s.end);
        }
        if pos < source.len() && !source.get(pos..).unwrap_or("x").trim().is_empty() {
            out.push(Span::new(pos, source.len()));
        }
        out
    }

    /// Rebuilds the file from its parsed pieces and the whitespace between them (P1.15).
    /// Equal to `source` byte for byte when nothing is lost.
    pub fn rebuild(&self, source: &str) -> String {
        let mut out = String::with_capacity(source.len());
        let mut pos = 0;
        for s in self.covered_spans() {
            out.push_str(source.get(pos..s.start).unwrap_or(""));
            out.push_str(s.slice(source));
            pos = s.end;
        }
        out.push_str(source.get(pos..).unwrap_or(""));
        out
    }

    /// Every block in document order (preamble first), depth first.
    pub fn all_blocks(&self) -> Vec<&Block> {
        fn walk<'a>(b: &'a Block, out: &mut Vec<&'a Block>) {
            out.push(b);
            for c in &b.children {
                walk(c, out);
            }
        }
        let mut out = Vec::new();
        if let Some(p) = &self.preamble {
            out.push(p);
        }
        for b in &self.blocks {
            walk(b, &mut out);
        }
        out
    }
}
