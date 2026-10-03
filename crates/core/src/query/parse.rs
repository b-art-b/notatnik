//! Query text → AST, with line/column errors and suggestions (QV-10).

use std::collections::HashSet;
use std::fmt;

use serde::Serialize;

use super::ast::*;
use crate::{TaskState, names::PageName, task::parse_date};

/// A problem in query text. `line` and `column` are 1-based (column counts characters).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct QueryError {
    /// Line (1-based).
    pub line: usize,
    /// Column (1-based).
    pub column: usize,
    /// What is wrong.
    pub message: String,
    /// A possible fix, e.g. "did you mean `tag`?".
    pub suggestion: Option<String>,
}

impl fmt::Display for QueryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}: {}", self.line, self.column, self.message)?;
        if let Some(s) = &self.suggestion {
            write!(f, "; {s}")?;
        }
        Ok(())
    }
}

impl std::error::Error for QueryError {}

const KEYS: [&str; 14] = [
    "task",
    "tag",
    "links",
    "ns",
    "page",
    "scheduled",
    "deadline",
    "text",
    "type",
    "sort",
    "limit",
    "group",
    "view",
    "columns",
];

struct Problem {
    offset: usize,
    message: String,
    suggestion: Option<String>,
}

fn problem(offset: usize, message: impl Into<String>) -> Problem {
    Problem {
        offset,
        message: message.into(),
        suggestion: None,
    }
}

fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut cur = vec![i + 1];
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            cur.push((prev[j] + cost).min(prev[j + 1] + 1).min(cur[j] + 1));
        }
        prev = cur;
    }
    prev[b.len()]
}

fn suggest(word: &str, candidates: &[&str]) -> Option<String> {
    let w = word.to_lowercase();
    candidates
        .iter()
        .map(|c| (levenshtein(&w, c), *c))
        .filter(|(d, c)| *d <= 2 && *d < c.len())
        .min_by_key(|(d, _)| *d)
        .map(|(_, c)| format!("did you mean `{c}`?"))
}

/// Splits on commas outside `[[ ]]`; returns trimmed parts with their byte offsets in `s`.
fn split_values(s: &str) -> Vec<(usize, &str)> {
    let b = s.as_bytes();
    let mut parts = Vec::new();
    let (mut depth, mut start, mut i) = (0usize, 0usize, 0usize);
    let mut push = |from: usize, to: usize| {
        let raw = s.get(from..to).unwrap_or("");
        let lead = raw.len() - raw.trim_start().len();
        parts.push((from + lead, raw.trim()));
    };
    while i < b.len() {
        match (b.get(i), b.get(i + 1)) {
            (Some(b'['), Some(b'[')) => {
                depth += 1;
                i += 2;
            }
            (Some(b']'), Some(b']')) => {
                depth = depth.saturating_sub(1);
                i += 2;
            }
            (Some(b','), _) if depth == 0 => {
                push(start, i);
                start = i + 1;
                i += 1;
            }
            _ => i += 1,
        }
    }
    push(start, s.len());
    parts
}

fn parse_name(raw: &str, offset: usize) -> Result<String, Problem> {
    let t = raw.trim();
    let t = t
        .strip_prefix("[[")
        .and_then(|x| x.strip_suffix("]]"))
        .unwrap_or(t);
    let t = t.strip_prefix('#').unwrap_or(t);
    PageName::parse(t)
        .map(|n| n.as_str().to_string())
        .map_err(|e| problem(offset, format!("invalid page name `{raw}`: {e}")))
}

/// Parses a date expression: `2026-10-03`, `today`, `+7d`, `this-week`, ...
pub fn parse_date_expr(s: &str) -> Option<DateExpr> {
    let s = s.trim();
    if let Some(d) = parse_date(s) {
        return Some(DateExpr::Abs(d));
    }
    let rel = match s {
        "today" => RelDate::Today,
        "yesterday" => RelDate::Yesterday,
        "tomorrow" => RelDate::Tomorrow,
        "this-week" => RelDate::ThisWeek,
        "next-week" => RelDate::NextWeek,
        "this-month" => RelDate::ThisMonth,
        _ => {
            let sign = match s.chars().next()? {
                '+' => 1,
                '-' => -1,
                _ => return None,
            };
            let unit = match s.chars().last()? {
                'd' => DateUnit::Day,
                'w' => DateUnit::Week,
                'm' => DateUnit::Month,
                'y' => DateUnit::Year,
                _ => return None,
            };
            let digits = s.get(1..s.len() - 1)?;
            if digits.is_empty() || digits.len() > 6 || !digits.bytes().all(|b| b.is_ascii_digit())
            {
                return None;
            }
            RelDate::Offset {
                n: sign * digits.parse::<i32>().ok()?,
                unit,
            }
        }
    };
    Some(DateExpr::Rel(rel))
}

const DATE_HELP: &str =
    "use YYYY-MM-DD, today, tomorrow, +3d, -2w, +1m, this-week, next-week or this-month";

fn date_expr(s: &str, offset: usize) -> Result<DateExpr, Problem> {
    parse_date_expr(s).ok_or_else(|| Problem {
        offset,
        message: format!("invalid date `{}`", s.trim()),
        suggestion: Some(DATE_HELP.to_string()),
    })
}

fn leading_op<'a>(s: &'a str, ops: &[&'static str]) -> Option<(&'static str, &'a str)> {
    ops.iter()
        .find_map(|op| s.strip_prefix(op).map(|r| (*op, r)))
}

fn date_cond(part: &str, offset: usize) -> Result<DateCond, Problem> {
    match part {
        "none" => {
            return Ok(DateCond {
                op: DateOp::None,
                a: None,
                b: None,
            });
        }
        "any" => {
            return Ok(DateCond {
                op: DateOp::Any,
                a: None,
                b: None,
            });
        }
        _ => {}
    }
    if let Some((op, rest)) = leading_op(part, &["<=", ">=", "<", ">", "="]) {
        let r = rest.trim_start();
        if r.starts_with(['<', '>', '=', '!', '~']) {
            return Err(problem(
                offset,
                format!(
                    "unknown operator `{}`",
                    part.get(..part.len() - r.len()).unwrap_or(op).trim()
                ),
            ));
        }
        let off = offset + part.len() - r.len();
        let a = date_expr(r, off)?;
        let op = match op {
            "<=" => DateOp::Le,
            ">=" => DateOp::Ge,
            "<" => DateOp::Lt,
            ">" => DateOp::Gt,
            _ => DateOp::Eq,
        };
        return Ok(DateCond {
            op,
            a: Some(a),
            b: None,
        });
    }
    if part.starts_with(['!', '~', '<', '>', '=']) {
        let op: String = part.chars().take_while(|c| "<>=!~".contains(*c)).collect();
        return Err(problem(offset, format!("unknown operator `{op}`")));
    }
    if let Some((l, r)) = part.split_once("..") {
        let a = date_expr(l, offset)?;
        let b = date_expr(r, offset + l.len() + 2)?;
        return Ok(DateCond {
            op: DateOp::Range,
            a: Some(a),
            b: Some(b),
        });
    }
    Ok(DateCond {
        op: DateOp::Eq,
        a: Some(date_expr(part, offset)?),
        b: None,
    })
}

fn prop_cond(part: &str, offset: usize) -> Result<PropCond, Problem> {
    match part {
        "none" => {
            return Ok(PropCond {
                op: CmpOp::Eq,
                value: PropOperand::None,
            });
        }
        "any" => {
            return Ok(PropCond {
                op: CmpOp::Eq,
                value: PropOperand::Any,
            });
        }
        _ => {}
    }
    let (op, rest) = match leading_op(part, &["!=", "<=", ">=", "<", ">", "="]) {
        Some((o, r)) => (
            match o {
                "!=" => CmpOp::Ne,
                "<=" => CmpOp::Le,
                ">=" => CmpOp::Ge,
                "<" => CmpOp::Lt,
                ">" => CmpOp::Gt,
                _ => CmpOp::Eq,
            },
            r.trim_start(),
        ),
        None => match part.strip_prefix("contains ") {
            Some(r) => (CmpOp::Contains, r.trim_start()),
            None => (CmpOp::Eq, part),
        },
    };
    let off = offset + part.len() - rest.len();
    if rest.is_empty() {
        return Err(problem(off, "missing value after the operator"));
    }
    if rest.starts_with(['<', '>', '=', '!', '~']) && op != CmpOp::Eq
        || (part.starts_with(['~']) && rest == part)
    {
        let shown: String = part.chars().take_while(|c| "<>=!~".contains(*c)).collect();
        return Err(problem(offset, format!("unknown operator `{shown}`")));
    }
    let value = if let Some(l) = rest.strip_prefix("[[").and_then(|x| x.strip_suffix("]]")) {
        PropOperand::Link(parse_name(l, off)?)
    } else if let Some(n) = number_text(rest) {
        PropOperand::Number(n)
    } else if let Some(d) = parse_date_expr(rest) {
        PropOperand::Date(d)
    } else {
        PropOperand::Text(rest.to_string())
    };
    Ok(PropCond { op, value })
}

fn number_text(s: &str) -> Option<f64> {
    let t = s.strip_prefix('-').unwrap_or(s);
    let ok = match t.split_once('.') {
        Some((i, f)) => {
            !i.is_empty()
                && !f.is_empty()
                && i.bytes().all(|b| b.is_ascii_digit())
                && f.bytes().all(|b| b.is_ascii_digit())
        }
        None => !t.is_empty() && t.bytes().all(|b| b.is_ascii_digit()),
    };
    if ok { s.parse().ok() } else { None }
}

fn valid_key(k: &str) -> bool {
    !k.is_empty()
        && k.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
}

fn sort_field(s: &str) -> Option<SortField> {
    Some(match s {
        "deadline" => SortField::Deadline,
        "scheduled" => SortField::Scheduled,
        "priority" => SortField::Priority,
        "page" => SortField::Page,
        "text" => SortField::Text,
        "task" => SortField::Task,
        _ => SortField::Prop(
            s.strip_prefix("prop.")
                .filter(|k| valid_key(k))?
                .to_string(),
        ),
    })
}

const SORT_FIELDS: [&str; 6] = ["deadline", "scheduled", "priority", "page", "text", "task"];

/// Parses query text. Returns a best-effort query (invalid clauses skipped) and all errors.
pub fn parse_query_partial(text: &str) -> (Query, Vec<QueryError>) {
    let mut q = Query::default();
    let mut errors: Vec<QueryError> = Vec::new();
    let mut seen: HashSet<&'static str> = HashSet::new();

    for (ln, raw) in text.lines().enumerate() {
        let line_no = ln + 1;
        let line = raw.trim_end_matches('\r');
        let lead = line.len() - line.trim_start().len();
        let body = line.trim();
        if body.is_empty() {
            continue;
        }
        let col = |off: usize| line.get(..lead + off).map_or(1, |p| p.chars().count()) + 1;
        let mut err = |off: usize, p: Problem| {
            errors.push(QueryError {
                line: line_no,
                column: col(off + p.offset),
                message: p.message,
                suggestion: p.suggestion,
            });
        };
        let negated = body.starts_with('-');
        let key_off = usize::from(negated);
        let Some(colon) = body.find(':') else {
            err(
                0,
                Problem {
                    offset: 0,
                    message: format!("expected `key: value`, found `{body}`"),
                    suggestion: suggest(body, &KEYS),
                },
            );
            continue;
        };
        let key = body.get(key_off..colon).unwrap_or("").trim();
        let value_raw = body.get(colon + 1..).unwrap_or("");
        let vlead = value_raw.len() - value_raw.trim_start().len();
        let voff = colon + 1 + vlead;
        let value = value_raw.trim();

        let single: Option<&'static str> = KEYS.iter().copied().find(|k| {
            *k == key && ["type", "sort", "limit", "group", "view", "columns"].contains(k)
        });
        if let Some(k) = single {
            if negated {
                err(
                    0,
                    problem(0, format!("`-` negation does not apply to `{k}`")),
                );
                continue;
            }
            if !seen.insert(k) {
                err(0, problem(key_off, format!("duplicate `{k}` clause")));
                continue;
            }
        }
        if value.is_empty() {
            err(0, problem(colon + 1, format!("empty value for `{key}`")));
            continue;
        }
        let parts = split_values(value);
        if parts.iter().any(|(_, p)| p.is_empty()) {
            err(
                voff,
                problem(0, format!("empty value in the list for `{key}`")),
            );
            continue;
        }

        // Collect per-part results; push the first error only.
        macro_rules! each {
            ($f:expr) => {{
                let mut out = Vec::new();
                let mut failed = false;
                for (o, p) in &parts {
                    match $f(*p, *o) {
                        Ok(v) => out.push(v),
                        Err(pr) => {
                            err(voff, pr);
                            failed = true;
                            break;
                        }
                    }
                }
                if failed {
                    continue;
                }
                out
            }};
        }

        if negated || !["type", "sort", "limit", "group", "view", "columns"].contains(&key) {
            let neg = negated;
            let kind = match key {
                "task" => {
                    FilterKind::Task(each!(|p: &str, o: usize| match p.to_lowercase().as_str() {
                        "any" => Ok(TaskMatch::Any),
                        "none" => Ok(TaskMatch::None),
                        s => TaskState::from_name(s)
                            .map(TaskMatch::State)
                            .ok_or_else(|| Problem {
                                offset: o,
                                message: format!("unknown task state `{p}`"),
                                suggestion: Some(
                                    "use todo, doing, waiting, done, canceled, any or none"
                                        .to_string()
                                ),
                            }),
                    }))
                }
                "tag" => FilterKind::Tag(each!(|p: &str, o: usize| parse_name(p, o))),
                "links" => FilterKind::Links(each!(|p: &str, o: usize| parse_name(p, o))),
                "ns" => FilterKind::Ns(each!(|p: &str, o: usize| parse_name(p, o))),
                "page" => FilterKind::Page(each!(|p: &str, o: usize| parse_name(p, o))),
                "text" => FilterKind::Text(each!(|p: &str, _o: usize| Ok::<String, Problem>(
                    p.to_string()
                ))),
                "scheduled" => FilterKind::Scheduled(each!(|p: &str, o: usize| date_cond(p, o))),
                "deadline" => FilterKind::Deadline(each!(|p: &str, o: usize| date_cond(p, o))),
                k if k.starts_with("prop.") => {
                    let pk = k.get(5..).unwrap_or("");
                    if !valid_key(pk) {
                        err(
                            0,
                            problem(
                                key_off + 5,
                                format!("invalid property key `{pk}` (use a-z, 0-9, _ and -)"),
                            ),
                        );
                        continue;
                    }
                    FilterKind::Prop {
                        key: pk.to_string(),
                        conds: each!(|p: &str, o: usize| prop_cond(p, o)),
                    }
                }
                other => {
                    let mut sug = suggest(other, &KEYS);
                    if other.starts_with("prop") && !other.starts_with("prop.") {
                        sug = Some("property filters are written `prop.<key>: value`".to_string());
                    }
                    err(
                        0,
                        Problem {
                            offset: key_off,
                            message: format!("unknown key `{other}`"),
                            suggestion: sug,
                        },
                    );
                    continue;
                }
            };
            q.filters.push(Filter { negated: neg, kind });
            continue;
        }

        match key {
            "type" => match value {
                "block" => q.target = QueryTarget::Block,
                "page" => q.target = QueryTarget::Page,
                _ => err(
                    voff,
                    Problem {
                        offset: 0,
                        message: format!("unknown type `{value}`"),
                        suggestion: Some("use block or page".into()),
                    },
                ),
            },
            "limit" => match value.parse::<usize>() {
                Ok(n) if n > 0 => q.limit = Some(n),
                _ => err(
                    voff,
                    problem(
                        0,
                        format!("limit must be a positive number, found `{value}`"),
                    ),
                ),
            },
            "view" => {
                if value.bytes().all(|b| {
                    b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'_' | b'-' | b'.')
                }) {
                    q.view = Some(value.to_string());
                } else {
                    err(voff, problem(0, format!("invalid view name `{value}`")));
                }
            }
            "columns" => {
                let mut cols = Vec::new();
                for (o, p) in &parts {
                    if p.bytes()
                        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.'))
                    {
                        cols.push((*p).to_string());
                    } else {
                        err(voff, problem(*o, format!("invalid column name `{p}`")));
                    }
                }
                q.columns = cols;
            }
            "group" => {
                let g = match value {
                    "page" => Some(GroupBy::Page),
                    "task" => Some(GroupBy::Task),
                    "scheduled" => Some(GroupBy::Scheduled),
                    "deadline" => Some(GroupBy::Deadline),
                    v => v
                        .strip_prefix("prop.")
                        .filter(|k| valid_key(k))
                        .map(|k| GroupBy::Prop(k.to_string())),
                };
                match g {
                    Some(g) => q.group = Some(g),
                    None => err(
                        voff,
                        Problem {
                            offset: 0,
                            message: format!("cannot group by `{value}`"),
                            suggestion: Some(
                                "use page, task, scheduled, deadline or prop.<key>".into(),
                            ),
                        },
                    ),
                }
            }
            "sort" => {
                let mut keys = Vec::new();
                let mut bad = false;
                for (o, p) in &parts {
                    let toks: Vec<&str> = p.split_whitespace().collect();
                    let field = toks.first().and_then(|f| sort_field(f));
                    let desc = match toks.get(1).copied() {
                        None | Some("asc") => Some(false),
                        Some("desc") => Some(true),
                        _ => None,
                    };
                    match (field, desc, toks.len()) {
                        (Some(f), Some(d), 1 | 2) => keys.push(SortKey { field: f, desc: d }),
                        (None, _, _) => {
                            err(
                                voff,
                                Problem {
                                    offset: *o,
                                    message: format!(
                                        "cannot sort by `{}`",
                                        toks.first().unwrap_or(&"")
                                    ),
                                    suggestion: suggest(toks.first().unwrap_or(&""), &SORT_FIELDS),
                                },
                            );
                            bad = true;
                        }
                        _ => {
                            err(
                                voff,
                                problem(
                                    *o,
                                    format!(
                                        "expected `asc` or `desc`, found `{}`",
                                        toks.get(1).unwrap_or(&"")
                                    ),
                                ),
                            );
                            bad = true;
                        }
                    }
                }
                if !bad {
                    q.sort = keys;
                }
            }
            _ => {}
        }
    }
    (q, errors)
}

/// Parses query text, failing with all errors found.
pub fn parse_query(text: &str) -> Result<Query, Vec<QueryError>> {
    let (q, errors) = parse_query_partial(text);
    if errors.is_empty() {
        Ok(q)
    } else {
        Err(errors)
    }
}
