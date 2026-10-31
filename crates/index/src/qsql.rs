//! Query AST → parameterised SQL, and running it (P1.27–P1.29).
//!
//! Values never end up in the SQL text: everything user-controlled is a bound parameter.

use std::collections::BTreeMap;
use std::fmt;

use chrono::NaiveDate;
use notatnik_core::query::*;
use notatnik_core::{PageName, search_fold};
use rusqlite::{params_from_iter, types::Value};
use serde::Serialize;

use crate::{Error, Index, Result, read::fts_match};

/// Injected context: "today" and the first day of the week. The index never reads the clock.
#[derive(Debug, Clone, Copy)]
pub struct QueryContext {
    /// The current date.
    pub today: NaiveDate,
    /// First day of the week.
    pub week_start: WeekStart,
}

/// A query that can't be compiled for the chosen target (e.g. `task:` with `type: page`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileError(pub String);

impl fmt::Display for CompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for CompileError {}

/// SQL text plus bound parameters.
#[derive(Debug, Clone, PartialEq)]
pub struct CompiledQuery {
    /// SQL with numbered placeholders.
    pub sql: String,
    /// Parameters, `?1` is `params[0]`.
    pub params: Vec<Value>,
    /// Blocks or pages.
    pub target: QueryTarget,
}

/// One result row.
#[derive(Debug, Clone, PartialEq, Serialize, Default)]
pub struct QueryRow {
    /// Stable identifier: `page#path` for blocks (path = child indices `0.2.1`, `p` = preamble), the page name for pages.
    pub id: String,
    /// Page name.
    pub page: String,
    /// File path of the page.
    pub file_path: Option<String>,
    /// Block row id (blocks only).
    pub block_id: Option<i64>,
    /// Block path (blocks only).
    pub path: Option<String>,
    /// Block `^id`.
    pub uid: Option<String>,
    /// Block text (raw Markdown, without ID).
    pub text: Option<String>,
    /// Task state (`todo`, ...).
    pub task_state: Option<String>,
    /// Scheduled date.
    pub scheduled: Option<String>,
    /// Scheduled time of day.
    pub scheduled_time: Option<String>,
    /// Deadline date.
    pub deadline: Option<String>,
    /// Deadline time of day.
    pub deadline_time: Option<String>,
    /// Priority.
    pub priority: Option<String>,
    /// Group label when the query has `group:`.
    pub group: Option<String>,
    /// The group value was written as a `[[link]]`.
    pub group_link: bool,
    /// Properties (key → first value), for table-like views.
    pub props: BTreeMap<String, String>,
    /// The keys of `props` whose value was written as a `[[link]]`.
    pub link_props: std::collections::BTreeSet<String>,
}

/// Query results.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct QueryResults {
    /// Rows in final order.
    pub rows: Vec<QueryRow>,
    /// Whether rows are blocks or pages.
    pub target: QueryTarget,
}

struct B {
    params: Vec<Value>,
}

impl B {
    fn p(&mut self, v: impl Into<Value>) -> String {
        self.params.push(v.into());
        format!("?{}", self.params.len())
    }
    fn list(&mut self, vals: &[String]) -> String {
        let ph: Vec<String> = vals.iter().map(|v| self.p(v.clone())).collect();
        format!("({})", ph.join(", "))
    }
}

fn key_of(name: &str) -> String {
    PageName::parse(name).map_or_else(|_| search_fold(name), |n| n.lookup_key())
}

fn or_join(parts: Vec<String>) -> String {
    match parts.len() {
        0 => "0".into(),
        1 => parts.into_iter().next().unwrap_or_default(),
        _ => format!("({})", parts.join(" OR ")),
    }
}

fn negate(neg: bool, e: String) -> String {
    if neg {
        format!("NOT COALESCE({e}, 0)")
    } else {
        e
    }
}

fn day(d: NaiveDate) -> String {
    d.format("%Y-%m-%d").to_string()
}

fn like_escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

fn date_cond_sql(col: &str, c: &DateCond, ctx: &QueryContext, b: &mut B) -> String {
    let r = |e: &DateExpr| e.resolve(ctx.today, ctx.week_start);
    match (c.op, c.a, c.b) {
        (DateOp::None, _, _) => format!("{col} IS NULL"),
        (DateOp::Any, _, _) => format!("{col} IS NOT NULL"),
        (DateOp::Eq, Some(a), _) => {
            let rg = r(&a);
            format!(
                "{col} BETWEEN {} AND {}",
                b.p(day(rg.start)),
                b.p(day(rg.end))
            )
        }
        (DateOp::Lt, Some(a), _) => format!("{col} < {}", b.p(day(r(&a).start))),
        (DateOp::Le, Some(a), _) => format!("{col} <= {}", b.p(day(r(&a).end))),
        (DateOp::Gt, Some(a), _) => format!("{col} > {}", b.p(day(r(&a).end))),
        (DateOp::Ge, Some(a), _) => format!("{col} >= {}", b.p(day(r(&a).start))),
        (DateOp::Range, Some(a), Some(z)) => format!(
            "{col} BETWEEN {} AND {}",
            b.p(day(r(&a).start)),
            b.p(day(r(&z).end))
        ),
        _ => "0".into(),
    }
}

fn prop_cond_sql(
    owner_kind: &str,
    owner: &str,
    key: &str,
    c: &PropCond,
    ctx: &QueryContext,
    b: &mut B,
) -> String {
    let base = |b: &mut B| {
        format!(
            "x.owner_kind = '{owner_kind}' AND x.key = {}",
            b.p(key.to_string())
        )
    };
    // Positive conditions drive from `props` (`owner IN (SELECT ...)`), so SQLite can start from the
    // few matching rows instead of probing every block.
    let exists = |cond: String| format!("{owner} IN (SELECT x.owner_id FROM props x WHERE {cond})");
    match (&c.value, c.op) {
        (PropOperand::Any, _) => {
            let k = base(b);
            exists(k)
        }
        (PropOperand::None, _) => {
            let k = base(b);
            format!("{owner} NOT IN (SELECT x.owner_id FROM props x WHERE {k})")
        }
        (v, CmpOp::Ne) => {
            let inner = prop_cond_sql(
                owner_kind,
                owner,
                key,
                &PropCond {
                    op: CmpOp::Eq,
                    value: v.clone(),
                },
                ctx,
                b,
            );
            format!("NOT {inner}")
        }
        (v, op) => {
            let k = base(b);
            let cmp = match (v, op) {
                (PropOperand::Number(n), CmpOp::Eq | CmpOp::Contains) => {
                    format!("x.value_num = {}", b.p(*n))
                }
                (PropOperand::Number(n), _) => format!("x.value_num {} {}", op.symbol(), b.p(*n)),
                (PropOperand::Date(d), op) => {
                    let rg = d.resolve(ctx.today, ctx.week_start);
                    match op {
                        CmpOp::Eq | CmpOp::Contains => {
                            let by_date = format!(
                                "x.value_date BETWEEN {} AND {}",
                                b.p(day(rg.start)),
                                b.p(day(rg.end))
                            );
                            match d {
                                // a property declared as text keeps no date column: match its text too
                                DateExpr::Abs(a) => {
                                    format!("({by_date} OR x.value_fold = {})", b.p(day(*a)))
                                }
                                DateExpr::Rel(_) => by_date,
                            }
                        }
                        CmpOp::Lt => format!("x.value_date < {}", b.p(day(rg.start))),
                        CmpOp::Le => format!("x.value_date <= {}", b.p(day(rg.end))),
                        CmpOp::Gt => format!("x.value_date > {}", b.p(day(rg.end))),
                        CmpOp::Ge => format!("x.value_date >= {}", b.p(day(rg.start))),
                        CmpOp::Ne => "0".into(),
                    }
                }
                (PropOperand::Text(t), CmpOp::Contains) => {
                    format!(
                        "x.value_fold LIKE {} ESCAPE '\\'",
                        b.p(format!("%{}%", like_escape(&search_fold(t))))
                    )
                }
                (PropOperand::Text(t), CmpOp::Eq) => {
                    format!("x.value_fold = {}", b.p(search_fold(t)))
                }
                (PropOperand::Text(t), _) => {
                    format!("x.value_fold {} {}", op.symbol(), b.p(search_fold(t)))
                }
                (PropOperand::Link(l), _) => format!("x.value_fold = {}", b.p(search_fold(l))),
                _ => "0".into(),
            };
            exists(format!("{k} AND {cmp}"))
        }
    }
}

/// A block property that applies to everything below it (`inherit:`). The nearest block with the key decides
/// (its own value wins over a parent's); a page's frontmatter is the root for blocks without one in their chain.
fn inherited_prop_sql(key: &str, c: &PropCond, ctx: &QueryContext, b: &mut B) -> String {
    match (&c.value, c.op) {
        (PropOperand::None, _) => {
            let any = PropCond {
                op: CmpOp::Eq,
                value: PropOperand::Any,
            };
            format!("NOT {}", inherited_prop_sql(key, &any, ctx, b))
        }
        (v, CmpOp::Ne) => {
            let eq = PropCond {
                op: CmpOp::Eq,
                value: v.clone(),
            };
            format!("NOT {}", inherited_prop_sql(key, &eq, ctx, b))
        }
        _ => {
            // `eff(id, src)`: every block below (or being) a block that has the key, and the block it takes the
            // value from. A block with its own value starts a new chain instead of continuing its parent's.
            let eff = |b: &mut B| {
                format!(
                    "WITH RECURSIVE eff(id, src) AS (SELECT o.owner_id, o.owner_id FROM props o WHERE o.owner_kind = 'block' AND o.key = {k} UNION SELECT c.id, e.src FROM blocks c JOIN eff e ON c.parent_id = e.id WHERE NOT EXISTS (SELECT 1 FROM props o WHERE o.owner_kind = 'block' AND o.owner_id = c.id AND o.key = {k}))",
                    k = b.p(key.to_string())
                )
            };
            let with = eff(b);
            let own = prop_cond_sql("block", "src", key, c, ctx, b);
            let with2 = eff(b);
            let page = prop_cond_sql("page", "b.page_id", key, c, ctx, b);
            format!(
                "(b.id IN ({with} SELECT id FROM eff WHERE {own}) OR (b.id NOT IN ({with2} SELECT id FROM eff) AND {page}))"
            )
        }
    }
}

fn task_rank(col: &str) -> String {
    format!(
        "CASE {col} WHEN 'doing' THEN 0 WHEN 'now' THEN 0 WHEN 'todo' THEN 1 WHEN 'later' THEN 1 WHEN 'waiting' THEN 2 WHEN 'done' THEN 3 WHEN 'canceled' THEN 4 ELSE 5 END"
    )
}

fn prop_value_expr(owner_kind: &str, owner: &str, key: &str, b: &mut B) -> String {
    format!(
        "(SELECT COALESCE(x.value_num, x.value_date, x.value_fold) FROM props x WHERE x.owner_kind = '{owner_kind}' AND x.owner_id = {owner} AND x.key = {} ORDER BY x.ord LIMIT 1)",
        b.p(key.to_string())
    )
}

/// Compiles a query. `keys_for` expands a page name into its lookup key plus alias keys.
pub fn compile(
    q: &Query,
    ctx: &QueryContext,
    keys_for: &dyn Fn(&str) -> Vec<String>,
) -> std::result::Result<CompiledQuery, CompileError> {
    let mut b = B { params: Vec::new() };
    let pages = q.target == QueryTarget::Page;
    let (owner_kind, owner, page_alias) = if pages {
        ("page", "p.id", "p.id")
    } else {
        ("block", "b.id", "b.page_id")
    };
    let mut wheres: Vec<String> = Vec::new();
    if pages {
        wheres.push("p.is_virtual = 0".into());
    }
    let only_blocks = |what: &str| {
        CompileError(format!(
            "`{what}:` only works on blocks, not with `type: page`"
        ))
    };

    for f in &q.filters {
        let expr = match &f.kind {
            FilterKind::Task(vals) => {
                if pages {
                    return Err(only_blocks("task"));
                }
                let mut parts = Vec::new();
                let states: Vec<String> = vals
                    .iter()
                    .filter_map(|v| {
                        if let TaskMatch::State(s) = v {
                            Some(s.name().to_string())
                        } else {
                            None
                        }
                    })
                    .collect();
                if !states.is_empty() {
                    parts.push(format!("b.task_state IN {}", b.list(&states)));
                }
                if vals.contains(&TaskMatch::Any) {
                    parts.push("b.task_state IS NOT NULL".into());
                }
                if vals.contains(&TaskMatch::None) {
                    parts.push("b.task_state IS NULL".into());
                }
                or_join(parts)
            }
            FilterKind::Tag(names) | FilterKind::Links(names) => {
                let is_tag = matches!(f.kind, FilterKind::Tag(_));
                let mut keys: Vec<String> = Vec::new();
                for n in names {
                    for k in keys_for(n) {
                        if !keys.contains(&k) {
                            keys.push(k);
                        }
                    }
                }
                let kinds = if is_tag {
                    "'tag'"
                } else {
                    "'page','tag','page_embed'"
                };
                let list = b.list(&keys);
                if pages {
                    format!(
                        "EXISTS (SELECT 1 FROM refs r WHERE r.src_page_id = p.id AND {} r.kind IN ({kinds}) AND r.target_key IN {list})",
                        if is_tag {
                            "r.src_block_id IS NULL AND"
                        } else {
                            ""
                        }
                    )
                } else {
                    // blocks with the link themselves, plus (unless `direct: yes`) everything below them: a link
                    // applies to the subtree (LN-11)
                    let reach = if q.direct {
                        format!(
                            "SELECT r.src_block_id FROM refs r WHERE r.kind IN ({kinds}) AND r.src_block_id IS NOT NULL AND r.target_key IN {list}"
                        )
                    } else {
                        format!(
                            "WITH RECURSIVE sub(id) AS (SELECT r.src_block_id FROM refs r WHERE r.kind IN ({kinds}) AND r.src_block_id IS NOT NULL AND r.target_key IN {list} UNION SELECT c.id FROM blocks c JOIN sub s ON c.parent_id = s.id) SELECT id FROM sub"
                        )
                    };
                    let page_tag = if is_tag {
                        format!(
                            " UNION SELECT bb.id FROM blocks bb WHERE bb.page_id IN (SELECT r.src_page_id FROM refs r WHERE r.kind = 'tag' AND r.src_block_id IS NULL AND r.target_key IN {list})"
                        )
                    } else {
                        String::new()
                    };
                    format!("b.id IN ({reach}{page_tag})")
                }
            }
            FilterKind::Chain(uids) => {
                if pages {
                    return Err(only_blocks("chain"));
                }
                let list = b.list(uids);
                // up to the first task of the chain, then down to everything that follows (UNION ends cycles)
                format!(
                    "b.id IN (WITH RECURSIVE up(id) AS (SELECT id FROM blocks WHERE uid IN {list} \
                     UNION SELECT t.id FROM up JOIN props pr ON pr.owner_kind = 'block' AND pr.owner_id = up.id AND pr.key = 'follows' \
                     JOIN blocks t ON '((' || t.uid || '))' = pr.value_text), \
                     down(id) AS (SELECT id FROM up \
                     UNION SELECT pr.owner_id FROM down JOIN blocks s ON s.id = down.id \
                     JOIN props pr ON pr.owner_kind = 'block' AND pr.key = 'follows' AND pr.value_text = '((' || s.uid || '))') \
                     SELECT id FROM down)"
                )
            }
            FilterKind::Ns(names) => {
                let parts: Vec<String> = names
                    .iter()
                    .map(|n| {
                        let k = key_of(n);
                        let lo = format!("{k}/");
                        let hi = format!("{k}0");
                        format!(
                            "(p.key = {} OR (p.key >= {} AND p.key < {}))",
                            b.p(k),
                            b.p(lo),
                            b.p(hi)
                        )
                    })
                    .collect();
                or_join(parts)
            }
            FilterKind::Path(names) => {
                // `/a/b/` inside `/<key>/`: whole segments, next to each other (LIKE wildcards in names are escaped)
                let esc = |s: &str| {
                    s.replace('\\', "\\\\")
                        .replace('%', "\\%")
                        .replace('_', "\\_")
                };
                let parts: Vec<String> = names
                    .iter()
                    .map(|n| {
                        let pat = format!("%/{}/%", esc(&key_of(n)));
                        format!("('/' || p.key || '/') LIKE {} ESCAPE '\\'", b.p(pat))
                    })
                    .collect();
                or_join(parts)
            }
            FilterKind::Page(names) => {
                let keys: Vec<String> = names.iter().map(|n| key_of(n)).collect();
                format!("p.key IN {}", b.list(&keys))
            }
            FilterKind::Prop { key, conds } => {
                let inherit = !pages && !q.direct && q.inherit.iter().any(|k| k == key);
                let parts: Vec<String> = conds
                    .iter()
                    .map(|c| {
                        if inherit {
                            inherited_prop_sql(key, c, ctx, &mut b)
                        } else {
                            prop_cond_sql(owner_kind, owner, key, c, ctx, &mut b)
                        }
                    })
                    .collect();
                or_join(parts)
            }
            FilterKind::Scheduled(conds) | FilterKind::Deadline(conds) => {
                let (name, col) = if matches!(f.kind, FilterKind::Scheduled(_)) {
                    ("scheduled", "b.scheduled")
                } else {
                    ("deadline", "b.deadline")
                };
                if pages {
                    return Err(only_blocks(name));
                }
                let parts: Vec<String> = conds
                    .iter()
                    .map(|c| date_cond_sql(col, c, ctx, &mut b))
                    .collect();
                or_join(parts)
            }
            FilterKind::Priority(labels) => {
                if pages {
                    return Err(only_blocks("priority"));
                }
                let names: Vec<String> = labels.iter().map(|l| l.trim().to_lowercase()).collect();
                format!("lower(b.priority) IN {}", b.list(&names))
            }
            FilterKind::Text(terms) => {
                let exprs: Vec<String> = terms
                    .iter()
                    .filter_map(|t| fts_match(t))
                    .map(|m| format!("({m})"))
                    .collect();
                if exprs.is_empty() {
                    "0".into()
                } else if pages {
                    format!(
                        "p.id IN (SELECT -rowid FROM fts WHERE rowid < 0 AND fts MATCH {})",
                        b.p(exprs.join(" OR "))
                    )
                } else {
                    format!(
                        "b.id IN (SELECT rowid FROM fts WHERE rowid > 0 AND fts MATCH {})",
                        b.p(exprs.join(" OR "))
                    )
                }
            }
        };
        wheres.push(negate(f.negated, expr));
    }
    let _ = page_alias;

    // grouping and ordering
    let mut select_group = "NULL".to_string();
    // 1 when the group value was written as a `[[link]]` (the label alone does not say), else 0
    let mut select_link = "0".to_string();
    let mut order: Vec<String> = Vec::new();
    if let Some(g) = &q.group {
        match g {
            GroupBy::Page => {
                select_group = "p.name".into();
                order.push("p.key".into());
            }
            GroupBy::Task => {
                if pages {
                    return Err(only_blocks("group: task"));
                }
                select_group = "b.task_state".into();
                order.push(task_rank("b.task_state"));
            }
            GroupBy::Scheduled | GroupBy::Deadline => {
                if pages {
                    return Err(only_blocks("group: scheduled/deadline"));
                }
                let col = if *g == GroupBy::Scheduled {
                    "b.scheduled"
                } else {
                    "b.deadline"
                };
                select_group = col.into();
                order.push(format!("{col} IS NULL"));
                order.push(col.into());
            }
            GroupBy::Prop(k) => {
                let sel = format!(
                    "(SELECT x.value_text FROM props x WHERE x.owner_kind = '{owner_kind}' AND x.owner_id = {owner} AND x.key = {} ORDER BY x.ord LIMIT 1)",
                    b.p(k.clone())
                );
                select_link = format!(
                    "COALESCE((SELECT x.is_link FROM props x WHERE x.owner_kind = '{owner_kind}' AND x.owner_id = {owner} AND x.key = {} ORDER BY x.ord LIMIT 1), 0)",
                    b.p(k.clone())
                );
                select_group = sel.clone();
                order.push(format!("{sel} IS NULL"));
                order.push(format!("{sel} COLLATE NOCASE"));
            }
        }
    }
    for k in &q.sort {
        let dir = if k.desc { "DESC" } else { "ASC" };
        match &k.field {
            SortField::Deadline | SortField::Scheduled => {
                if pages {
                    return Err(only_blocks("sort by dates"));
                }
                let (d, t) = if k.field == SortField::Deadline {
                    ("b.deadline", "b.deadline_time")
                } else {
                    ("b.scheduled", "b.scheduled_time")
                };
                order.push(format!("{d} IS NULL"));
                order.push(format!("{d} {dir}"));
                order.push(format!("{t} IS NULL"));
                order.push(format!("{t} {dir}"));
            }
            SortField::Priority => {
                if pages {
                    return Err(only_blocks("sort by priority"));
                }
                order.push("b.priority_rank IS NULL".into());
                order.push(format!("b.priority_rank {dir}"));
            }
            SortField::Page => order.push(format!("p.key {dir}")),
            SortField::Text => order.push(if pages {
                format!("p.key {dir}")
            } else {
                format!("b.text COLLATE NOCASE {dir}")
            }),
            SortField::Task => {
                if pages {
                    return Err(only_blocks("sort by task"));
                }
                order.push(format!("{} {dir}", task_rank("b.task_state")));
            }
            SortField::Prop(key) => {
                let e = prop_value_expr(owner_kind, owner, key, &mut b);
                order.push(format!("{e} IS NULL"));
                order.push(format!("{e} {dir}"));
            }
        }
    }
    let sql = if pages {
        order.push("p.key".into());
        format!(
            "SELECT p.id, p.name, p.file_path, {select_group} AS grp, {select_link} AS grp_link FROM pages p WHERE {} ORDER BY {}{}",
            wheres.join(" AND "),
            order.join(", "),
            q.limit.map_or(String::new(), |l| format!(" LIMIT {l}"))
        )
    } else {
        order.push("p.key".into());
        order.push("b.id".into());
        let where_sql = if wheres.is_empty() {
            "1".to_string()
        } else {
            wheres.join(" AND ")
        };
        format!(
            "SELECT b.id, b.page_id, p.name, p.file_path, b.path, b.uid, b.text, b.task_state, b.scheduled, b.scheduled_time, b.deadline, b.deadline_time, b.priority, {select_group} AS grp, {select_link} AS grp_link FROM blocks b JOIN pages p ON p.id = b.page_id WHERE {where_sql} ORDER BY {}{}",
            order.join(", "),
            q.limit.map_or(String::new(), |l| format!(" LIMIT {l}"))
        )
    };
    Ok(CompiledQuery {
        sql,
        params: b.params,
        target: q.target,
    })
}

impl Index {
    /// Compiles a query against this index (aliases are resolved here).
    pub fn compile_query(
        &self,
        q: &Query,
        ctx: &QueryContext,
    ) -> std::result::Result<CompiledQuery, CompileError> {
        compile(q, ctx, &|n| self.keys_for(n))
    }

    /// Runs a query and returns rows with properties filled in.
    pub fn run_query(&self, q: &Query, ctx: &QueryContext) -> Result<QueryResults> {
        let compiled = self.compile_query(q, ctx).map_err(|e| Error::Query(e.0))?;
        let mut st = self.conn.prepare(&compiled.sql)?;
        let mut rows: Vec<QueryRow> = Vec::new();
        let pages = compiled.target == QueryTarget::Page;
        let mapped = st.query_map(params_from_iter(compiled.params.iter()), |r| {
            if pages {
                let name: String = r.get(1)?;
                Ok(QueryRow {
                    id: name.clone(),
                    page: name,
                    file_path: r.get(2)?,
                    group: r.get(3)?,
                    group_link: r.get(4)?,
                    block_id: Some(-r.get::<_, i64>(0)?),
                    ..QueryRow::default()
                })
            } else {
                let page: String = r.get(2)?;
                let path: String = r.get(4)?;
                Ok(QueryRow {
                    id: format!("{page}#{path}"),
                    block_id: Some(r.get(0)?),
                    page,
                    file_path: r.get(3)?,
                    path: Some(path),
                    uid: r.get(5)?,
                    text: r.get(6)?,
                    task_state: r.get(7)?,
                    scheduled: r.get(8)?,
                    scheduled_time: r.get(9)?,
                    deadline: r.get(10)?,
                    deadline_time: r.get(11)?,
                    priority: r.get(12)?,
                    group: r.get(13)?,
                    group_link: r.get(14)?,
                    props: BTreeMap::new(),
                    link_props: Default::default(),
                })
            }
        })?;
        for row in mapped {
            rows.push(row?);
        }
        let kind = if pages { "page" } else { "block" };
        let mut pst = self.conn.prepare_cached("SELECT key, value_text, is_link FROM props WHERE owner_kind = ?1 AND owner_id = ?2 ORDER BY key, ord")?;
        for row in &mut rows {
            let Some(id) = row.block_id else { continue };
            let owner = if pages { -id } else { id };
            let list = pst.query_map(rusqlite::params![kind, owner], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, Option<String>>(1)?,
                    r.get::<_, bool>(2)?,
                ))
            })?;
            for p in list {
                let (k, v, link) = p?;
                // the first value of a key is the one shown; it is a link when it was written as `[[...]]`
                if !row.props.contains_key(&k) {
                    if link {
                        row.link_props.insert(k.clone());
                    }
                    row.props.insert(k, v.unwrap_or_default());
                }
            }
        }
        if pages {
            for row in &mut rows {
                row.block_id = None;
            }
        }
        Ok(QueryResults {
            rows,
            target: compiled.target,
        })
    }
}
