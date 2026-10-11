//! Query AST. `Query::to_text` gives the canonical text, and parsing it yields an equal AST.

use std::fmt::{self, Write};

use chrono::NaiveDate;
use serde::Serialize;

use crate::TaskState;

/// A parsed query.
#[derive(Debug, Clone, PartialEq, Serialize, Default)]
pub struct Query {
    /// Filters, ANDed, in source order.
    pub filters: Vec<Filter>,
    /// Whether to return blocks or pages.
    pub target: QueryTarget,
    /// Sort keys (earlier wins).
    pub sort: Vec<SortKey>,
    /// Maximum number of results.
    pub limit: Option<usize>,
    /// Grouping.
    pub group: Option<GroupBy>,
    /// View name (`list`, `table`, `kanban`, `timeline` or a plugin view).
    pub view: Option<String>,
    /// Columns for table-like views.
    pub columns: Vec<String>,
    /// `direct: yes`: `links:` and `tag:` match only blocks that carry the link themselves. By default a block
    /// also matches through its ancestors (a link applies to everything below it).
    pub direct: bool,
    /// `inherit: client, type`: these properties also apply to everything below the block that has them (the
    /// nearest block wins; the page's frontmatter is the root). Ignored with `direct: yes` and for page queries.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub inherit: Vec<String>,
}

/// What a query returns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum QueryTarget {
    /// Blocks (default).
    #[default]
    Block,
    /// Pages.
    Page,
}

/// One filter clause.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Filter {
    /// `-key:` negation.
    pub negated: bool,
    /// What it matches.
    pub kind: FilterKind,
}

/// Filter clause kinds. Multiple values in one clause are ORed.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", content = "values", rename_all = "snake_case")]
pub enum FilterKind {
    /// `task: todo, doing`
    Task(Vec<TaskMatch>),
    /// `tag: a, b` (normalised page names)
    Tag(Vec<String>),
    /// `links: page`
    Links(Vec<String>),
    /// `ns: work`: the page and everything below it.
    Ns(Vec<String>),
    /// `page: work`: only that page.
    Page(Vec<String>),
    /// `path: renewal`: pages that have this name as a segment anywhere in their path (`p/ey/renewal/x`,
    /// `x/renewal`); `a/b` matches the two segments next to each other.
    Path(Vec<String>),
    /// `priority: A, B`: tasks with one of these priority labels (compared without regard to case).
    Priority(Vec<String>),
    /// `prop.status: active`
    Prop {
        /// Property key.
        key: String,
        /// Alternatives (ORed).
        conds: Vec<PropCond>,
    },
    /// `scheduled: < +7d`
    Scheduled(Vec<DateCond>),
    /// `deadline: none`
    Deadline(Vec<DateCond>),
    /// `text: word`
    Text(Vec<String>),
}

/// A value of `task:`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskMatch {
    /// A specific state.
    State(TaskState),
    /// Any task.
    Any,
    /// Not a task.
    None,
}

/// Comparison operator for properties.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CmpOp {
    /// `=` (default)
    Eq,
    /// `!=`
    Ne,
    /// `<`
    Lt,
    /// `<=`
    Le,
    /// `>`
    Gt,
    /// `>=`
    Ge,
    /// `contains`
    Contains,
}

impl CmpOp {
    /// The query text of the operator.
    pub const fn symbol(self) -> &'static str {
        match self {
            CmpOp::Eq => "=",
            CmpOp::Ne => "!=",
            CmpOp::Lt => "<",
            CmpOp::Le => "<=",
            CmpOp::Gt => ">",
            CmpOp::Ge => ">=",
            CmpOp::Contains => "contains",
        }
    }
}

/// Right-hand side of a property comparison.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum PropOperand {
    /// Text.
    Text(String),
    /// Number.
    Number(f64),
    /// A date, absolute or relative.
    Date(DateExpr),
    /// A page link `[[x]]` (normalised name).
    Link(String),
    /// The property exists.
    Any,
    /// The property is missing.
    None,
}

/// One alternative of a `prop.<key>:` clause.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PropCond {
    /// Operator.
    pub op: CmpOp,
    /// Operand.
    pub value: PropOperand,
}

/// Comparison kind for date clauses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DateOp {
    /// Falls in the day / range of `a`.
    Eq,
    /// Before `a`.
    Lt,
    /// Up to and including `a`.
    Le,
    /// After `a`.
    Gt,
    /// `a` or later.
    Ge,
    /// From `a` to `b` inclusive.
    Range,
    /// Has a date.
    Any,
    /// Has no date.
    None,
}

/// One alternative of a `scheduled:` / `deadline:` clause.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DateCond {
    /// Kind.
    pub op: DateOp,
    /// First operand (absent for `any`/`none`).
    pub a: Option<DateExpr>,
    /// Second operand (ranges only).
    pub b: Option<DateExpr>,
}

/// A date, absolute or relative to "today".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum DateExpr {
    /// `2026-10-03`
    Abs(NaiveDate),
    /// `today`, `+7d`, `this-week`, ...
    Rel(RelDate),
}

/// Relative date forms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RelDate {
    /// `today`
    Today,
    /// `yesterday`
    Yesterday,
    /// `tomorrow`
    Tomorrow,
    /// `+3d`, `-2w`, `+1m`, `-1y`
    Offset {
        /// Signed amount.
        n: i32,
        /// Unit.
        unit: DateUnit,
    },
    /// `this-week` (a range)
    ThisWeek,
    /// `next-week` (a range)
    NextWeek,
    /// `this-month` (a range)
    ThisMonth,
}

/// Unit of a relative offset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DateUnit {
    /// `d`
    Day,
    /// `w`
    Week,
    /// `m`
    Month,
    /// `y`
    Year,
}

/// What a sort key orders by.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "field", content = "key", rename_all = "snake_case")]
pub enum SortField {
    /// Task deadline.
    Deadline,
    /// Task scheduled date.
    Scheduled,
    /// Task priority (high first when ascending).
    Priority,
    /// Page name.
    Page,
    /// Block text.
    Text,
    /// Task state.
    Task,
    /// A property.
    Prop(String),
}

/// One sort key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SortKey {
    /// Field.
    pub field: SortField,
    /// Descending order.
    pub desc: bool,
}

/// Grouping of results.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "by", content = "key", rename_all = "snake_case")]
pub enum GroupBy {
    /// By page.
    Page,
    /// By task state.
    Task,
    /// By scheduled date.
    Scheduled,
    /// By deadline.
    Deadline,
    /// By property value.
    Prop(String),
}

impl fmt::Display for DateExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DateExpr::Abs(d) => write!(f, "{}", d.format("%Y-%m-%d")),
            DateExpr::Rel(r) => match r {
                RelDate::Today => f.write_str("today"),
                RelDate::Yesterday => f.write_str("yesterday"),
                RelDate::Tomorrow => f.write_str("tomorrow"),
                RelDate::ThisWeek => f.write_str("this-week"),
                RelDate::NextWeek => f.write_str("next-week"),
                RelDate::ThisMonth => f.write_str("this-month"),
                RelDate::Offset { n, unit } => {
                    let u = match unit {
                        DateUnit::Day => 'd',
                        DateUnit::Week => 'w',
                        DateUnit::Month => 'm',
                        DateUnit::Year => 'y',
                    };
                    write!(
                        f,
                        "{}{}{u}",
                        if *n < 0 { '-' } else { '+' },
                        n.unsigned_abs()
                    )
                }
            },
        }
    }
}

fn name_text(n: &str) -> String {
    if n.contains(',') || n.starts_with(['#', '[']) {
        format!("[[{n}]]")
    } else {
        n.to_string()
    }
}

impl fmt::Display for DateCond {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (Some(a), b) = (self.a, self.b) else {
            return f.write_str(if self.op == DateOp::None {
                "none"
            } else {
                "any"
            });
        };
        match self.op {
            DateOp::Eq => write!(f, "{a}"),
            DateOp::Lt => write!(f, "< {a}"),
            DateOp::Le => write!(f, "<= {a}"),
            DateOp::Gt => write!(f, "> {a}"),
            DateOp::Ge => write!(f, ">= {a}"),
            DateOp::Range => match b {
                Some(b) => write!(f, "{a}..{b}"),
                None => write!(f, "{a}"),
            },
            DateOp::Any => f.write_str("any"),
            DateOp::None => f.write_str("none"),
        }
    }
}

impl fmt::Display for PropCond {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let operand = match &self.value {
            PropOperand::Any => return f.write_str("any"),
            PropOperand::None => return f.write_str("none"),
            PropOperand::Text(t) => t.clone(),
            PropOperand::Number(n) => n.to_string(),
            PropOperand::Date(d) => d.to_string(),
            PropOperand::Link(l) => format!("[[{l}]]"),
        };
        match self.op {
            CmpOp::Eq if !operand.starts_with(['<', '>', '=', '!']) => f.write_str(&operand),
            op => write!(f, "{} {operand}", op.symbol()),
        }
    }
}

impl Query {
    /// The canonical query text. Parsing it gives back an equal [`Query`].
    pub fn to_text(&self) -> String {
        let mut out = String::new();
        for fl in &self.filters {
            let neg = if fl.negated { "-" } else { "" };
            let (key, vals): (String, Vec<String>) = match &fl.kind {
                FilterKind::Task(v) => (
                    "task".into(),
                    v.iter()
                        .map(|t| match t {
                            TaskMatch::State(s) => s.name().to_string(),
                            TaskMatch::Any => "any".into(),
                            TaskMatch::None => "none".into(),
                        })
                        .collect(),
                ),
                FilterKind::Tag(v) => ("tag".into(), v.iter().map(|s| name_text(s)).collect()),
                FilterKind::Links(v) => ("links".into(), v.iter().map(|s| name_text(s)).collect()),
                FilterKind::Ns(v) => ("ns".into(), v.iter().map(|s| name_text(s)).collect()),
                FilterKind::Page(v) => ("page".into(), v.iter().map(|s| name_text(s)).collect()),
                FilterKind::Path(v) => ("path".into(), v.iter().map(|s| name_text(s)).collect()),
                FilterKind::Prop { key, conds } => (
                    format!("prop.{key}"),
                    conds.iter().map(ToString::to_string).collect(),
                ),
                FilterKind::Scheduled(v) => (
                    "scheduled".into(),
                    v.iter().map(ToString::to_string).collect(),
                ),
                FilterKind::Deadline(v) => (
                    "deadline".into(),
                    v.iter().map(ToString::to_string).collect(),
                ),
                FilterKind::Text(v) => ("text".into(), v.clone()),
                FilterKind::Priority(v) => ("priority".into(), v.clone()),
            };
            let _ = writeln!(out, "{neg}{key}: {}", vals.join(", "));
        }
        if self.target == QueryTarget::Page {
            out.push_str("type: page\n");
        }
        if !self.sort.is_empty() {
            let keys: Vec<String> = self
                .sort
                .iter()
                .map(|k| {
                    let name = match &k.field {
                        SortField::Deadline => "deadline".to_string(),
                        SortField::Scheduled => "scheduled".into(),
                        SortField::Priority => "priority".into(),
                        SortField::Page => "page".into(),
                        SortField::Text => "text".into(),
                        SortField::Task => "task".into(),
                        SortField::Prop(p) => format!("prop.{p}"),
                    };
                    format!("{name} {}", if k.desc { "desc" } else { "asc" })
                })
                .collect();
            let _ = writeln!(out, "sort: {}", keys.join(", "));
        }
        if let Some(g) = &self.group {
            let v = match g {
                GroupBy::Page => "page".to_string(),
                GroupBy::Task => "task".into(),
                GroupBy::Scheduled => "scheduled".into(),
                GroupBy::Deadline => "deadline".into(),
                GroupBy::Prop(p) => format!("prop.{p}"),
            };
            let _ = writeln!(out, "group: {v}");
        }
        if self.direct {
            let _ = writeln!(out, "direct: yes");
        }
        if !self.inherit.is_empty() {
            let _ = writeln!(out, "inherit: {}", self.inherit.join(", "));
        }
        if let Some(l) = self.limit {
            let _ = writeln!(out, "limit: {l}");
        }
        if let Some(v) = &self.view {
            let _ = writeln!(out, "view: {v}");
        }
        if !self.columns.is_empty() {
            let _ = writeln!(out, "columns: {}", self.columns.join(", "));
        }
        out
    }
}
