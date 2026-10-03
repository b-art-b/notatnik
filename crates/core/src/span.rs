//! Byte spans into the original source, and parser diagnostics.

use serde::Serialize;

/// A half-open byte range `start..end` into the original source text.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, serde::Deserialize, Default,
)]
pub struct Span {
    /// First byte (inclusive).
    pub start: usize,
    /// One past the last byte (exclusive).
    pub end: usize,
}

impl Span {
    /// Creates a span.
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    /// Length in bytes.
    pub const fn len(&self) -> usize {
        self.end.saturating_sub(self.start)
    }

    /// True when the span covers no bytes.
    pub const fn is_empty(&self) -> bool {
        self.end <= self.start
    }

    /// The text of this span in `source` (empty if out of range or not on char boundaries).
    pub fn slice<'a>(&self, source: &'a str) -> &'a str {
        source.get(self.start..self.end).unwrap_or("")
    }

    /// True when `other` lies completely inside `self`.
    pub const fn contains_span(&self, other: &Span) -> bool {
        self.start <= other.start && other.end <= self.end
    }

    /// The span shifted forward by `by` bytes.
    pub const fn shifted(&self, by: usize) -> Span {
        Span::new(self.start + by, self.end + by)
    }
}

/// How serious a [`Diagnostic`] is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// The construct could not be understood and was kept as text.
    Error,
    /// Understood, but probably not what the author meant.
    Warning,
    /// Informational.
    Info,
}

/// A problem found while parsing. The parser never fails; it reports these instead.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Diagnostic {
    /// Where the problem is.
    pub span: Span,
    /// Severity.
    pub severity: Severity,
    /// Stable machine-readable code, e.g. `invalid-date`.
    pub code: &'static str,
    /// Human-readable message.
    pub message: String,
}

impl Diagnostic {
    pub(crate) fn new(
        span: Span,
        severity: Severity,
        code: &'static str,
        message: impl Into<String>,
    ) -> Self {
        Self {
            span,
            severity,
            code,
            message: message.into(),
        }
    }
}
