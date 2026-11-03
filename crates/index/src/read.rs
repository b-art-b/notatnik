//! Read-side API: counts, page info, backlinks, full-text search, issues.

use std::collections::BTreeMap;

use notatnik_core::{PageName, fold_char, search_fold};
use rusqlite::{OptionalExtension, params, params_from_iter, types::Value};
use serde::Serialize;

use crate::{Index, Result};

/// Row counts of the index, used by tests (`expected-counts.json`) and `notatnik check`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct Counts {
    /// Pages with a file.
    pub pages: usize,
    /// Pages without a file (referenced or namespace parents).
    pub virtual_pages: usize,
    /// Blocks (including preambles).
    pub blocks: usize,
    /// Refs by kind (`page`, `tag`, `block`, `block_embed`, `page_embed`).
    pub refs: BTreeMap<String, usize>,
    /// Tasks by state (`todo`, `doing`, ...).
    pub tasks: BTreeMap<String, usize>,
}

/// Basic information about a page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PageInfo {
    /// Row id.
    pub id: i64,
    /// Display name.
    pub name: String,
    /// Frontmatter title.
    pub title: Option<String>,
    /// No file behind it.
    pub is_virtual: bool,
    /// Name is a date.
    pub is_journal: bool,
    /// Path of its file relative to the graph root.
    pub file_path: Option<String>,
    /// Aliases from frontmatter (as written, folded keys).
    pub aliases: Vec<String>,
}

/// One task of a chain (TK-15).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ChainTask {
    /// The block's `^id`, if it has one.
    pub uid: Option<String>,
    /// A `follows::` target that is not in the graph (deleted).
    pub missing: bool,
    /// Page name.
    pub page: String,
    /// Block path, `0.2`.
    pub path: String,
    /// Block text.
    pub text: String,
    /// `TODO`, `DONE`, ... (as indexed).
    pub task_state: Option<String>,
    /// Scheduled date.
    pub scheduled: Option<String>,
    /// Deadline.
    pub deadline: Option<String>,
    /// Nesting below the first task of the chain.
    pub depth: usize,
}

/// `((abc123))` → `abc123`.
fn block_ref_uid(v: &str) -> Option<String> {
    let inner = v.trim().strip_prefix("((")?.strip_suffix("))")?;
    (!inner.is_empty() && inner.bytes().all(|b| b.is_ascii_alphanumeric()))
        .then(|| inner.to_string())
}

/// A place that links to a page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Backlink {
    /// Page that contains the link.
    pub source_page: String,
    /// Block path (`0.2.1`); `None` for page-level links (frontmatter tags).
    pub block_path: Option<String>,
    /// Text of the linking block.
    pub text: String,
    /// First lines of the parent blocks, outermost first.
    pub breadcrumbs: Vec<String>,
    /// `page`, `tag` or `page_embed`.
    pub kind: String,
}

/// A block that mentions a page in plain text without linking to it (LN-4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Unlinked {
    /// Page that contains the mention.
    pub source_page: String,
    /// Block path (`0.2.1`).
    pub block_path: String,
    /// First lines of the parent blocks, outermost first.
    pub breadcrumbs: Vec<String>,
    /// The name or alias that matched.
    pub term: String,
}

/// What a search hit is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchKind {
    /// A page whose name/title/alias matched.
    Page,
    /// A block whose text matched.
    Block,
}

/// A full-text search result.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SearchHit {
    /// Page or block.
    pub kind: SearchKind,
    /// Page name.
    pub page: String,
    /// Block path, for block hits.
    pub path: Option<String>,
    /// Displayed text (block text or page name).
    pub text: String,
    /// Byte ranges in `text` to highlight.
    pub highlights: Vec<(usize, usize)>,
    /// bm25 score (lower is better).
    pub score: f64,
}

/// A problem recorded for a file (parse diagnostics, duplicate IDs, unreadable files).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IssueRow {
    /// File path.
    pub path: String,
    /// `error`, `warning`, `info`.
    pub severity: String,
    /// Machine-readable code.
    pub code: String,
    /// Message.
    pub message: String,
    /// Byte offset in the file, if known.
    pub offset: Option<i64>,
}

fn match_tokens(q: &str) -> Vec<(String, bool)> {
    q.split_whitespace()
        .filter_map(|t| {
            let prefix = t.ends_with('*');
            let t = search_fold(t.trim_end_matches('*').trim_matches('"'));
            let t = t.replace('"', "");
            if t.chars().any(char::is_alphanumeric) {
                Some((t, prefix))
            } else {
                None
            }
        })
        .collect()
}

/// Builds an FTS5 MATCH expression (all tokens must match; trailing `*` = prefix).
pub(crate) fn fts_match(q: &str) -> Option<String> {
    let toks = match_tokens(q);
    if toks.is_empty() {
        return None;
    }
    Some(
        toks.iter()
            .map(|(t, p)| format!("\"{t}\"{}", if *p { "*" } else { "" }))
            .collect::<Vec<_>>()
            .join(" "),
    )
}

fn highlights(text: &str, words: &[(String, bool)]) -> Vec<(usize, usize)> {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let folded: Vec<char> = chars.iter().map(|(_, c)| fold_char(*c)).collect();
    let mut out: Vec<(usize, usize)> = Vec::new();
    for (tok, prefix) in words.iter().map(|(t, p)| (t.as_str(), *p)) {
        let t: Vec<char> = tok.chars().collect();
        if t.is_empty() {
            continue;
        }
        let mut i = 0;
        while i + t.len() <= folded.len() {
            let word_start = i == 0 || !folded.get(i - 1).is_some_and(|c| c.is_alphanumeric());
            if word_start && folded.get(i..i + t.len()) == Some(&t[..]) {
                let mut end = i + t.len();
                if prefix {
                    while folded.get(end).is_some_and(|c| c.is_alphanumeric()) {
                        end += 1;
                    }
                }
                let s = chars.get(i).map_or(0, |c| c.0);
                let e = chars.get(end).map_or(text.len(), |c| c.0);
                out.push((s, e));
                i = end;
            } else {
                i += 1;
            }
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}

impl Index {
    /// Row counts.
    pub fn counts(&self) -> Result<Counts> {
        let count = |sql: &str| -> Result<usize> {
            Ok(self.conn.query_row(sql, [], |r| r.get::<_, i64>(0))? as usize)
        };
        let mut c = Counts {
            pages: count("SELECT COUNT(*) FROM pages WHERE is_virtual = 0")?,
            virtual_pages: count("SELECT COUNT(*) FROM pages WHERE is_virtual = 1")?,
            blocks: count("SELECT COUNT(*) FROM blocks")?,
            ..Counts::default()
        };
        let mut st = self
            .conn
            .prepare("SELECT kind, COUNT(*) FROM refs GROUP BY kind ORDER BY kind")?;
        for r in st.query_map([], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)? as usize))
        })? {
            let (k, n) = r?;
            c.refs.insert(k, n);
        }
        let mut st = self.conn.prepare("SELECT task_state, COUNT(*) FROM blocks WHERE task_state IS NOT NULL GROUP BY task_state ORDER BY task_state")?;
        for r in st.query_map([], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)? as usize))
        })? {
            let (k, n) = r?;
            c.tasks.insert(k, n);
        }
        Ok(c)
    }

    /// Looks a page up by name or alias (case-insensitive).
    pub fn page_info(&self, name: &str) -> Result<Option<PageInfo>> {
        let Ok(n) = PageName::parse(name) else {
            return Ok(None);
        };
        let Some(id) = self.resolve_page_id(&n.lookup_key())? else {
            return Ok(None);
        };
        let info = self
            .conn
            .query_row("SELECT id, name, title, is_virtual, is_journal, file_path FROM pages WHERE id = ?1", [id], |r| {
                Ok(PageInfo {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    title: r.get(2)?,
                    is_virtual: r.get::<_, i64>(3)? != 0,
                    is_journal: r.get::<_, i64>(4)? != 0,
                    file_path: r.get(5)?,
                    aliases: Vec::new(),
                })
            })
            .optional()?;
        let Some(mut info) = info else {
            return Ok(None);
        };
        let mut st = self
            .conn
            .prepare("SELECT alias_key FROM aliases WHERE page_id = ?1 ORDER BY alias_key")?;
        info.aliases = st
            .query_map([id], |r| r.get::<_, String>(0))?
            .collect::<std::result::Result<_, _>>()?;
        Ok(Some(info))
    }

    /// Resolves a lookup key to a page: real page first, then alias, then a virtual page.
    pub(crate) fn resolve_page_id(&self, key: &str) -> Result<Option<i64>> {
        let real: Option<i64> = self
            .conn
            .query_row(
                "SELECT id FROM pages WHERE key = ?1 AND is_virtual = 0",
                [key],
                |r| r.get(0),
            )
            .optional()?;
        if real.is_some() {
            return Ok(real);
        }
        let alias: Option<i64> = self
            .conn
            .query_row(
                "SELECT page_id FROM aliases WHERE alias_key = ?1 LIMIT 1",
                [key],
                |r| r.get(0),
            )
            .optional()?;
        if alias.is_some() {
            return Ok(alias);
        }
        Ok(self
            .conn
            .query_row("SELECT id FROM pages WHERE key = ?1", [key], |r| r.get(0))
            .optional()?)
    }

    /// All lookup keys that refer to the page `name`: its own key plus its aliases.
    pub fn keys_for(&self, name: &str) -> Vec<String> {
        let Ok(n) = PageName::parse(name) else {
            return vec![search_fold(name)];
        };
        let key = n.lookup_key();
        let mut keys = vec![key.clone()];
        if let Ok(Some(id)) = self.resolve_page_id(&key) {
            if let Ok(mut st) = self.conn.prepare_cached("SELECT key FROM pages WHERE id = ?1 UNION SELECT alias_key FROM aliases WHERE page_id = ?1") {
                if let Ok(rows) = st.query_map([id], |r| r.get::<_, String>(0)) {
                    for k in rows.flatten() {
                        if !keys.contains(&k) {
                            keys.push(k);
                        }
                    }
                }
            }
        }
        keys
    }

    /// Everything that links to `name` (page links, tags, page embeds; aliases included).
    pub fn backlinks(&self, name: &str) -> Result<Vec<Backlink>> {
        let keys = self.keys_for(name);
        let own = self.resolve_page_id(keys.first().map_or("", String::as_str))?;
        let ph = vec!["?"; keys.len()].join(",");
        let sql = format!(
            "SELECT DISTINCT r.src_page_id, r.src_block_id, r.kind, p.name, p.key
             FROM refs r JOIN pages p ON p.id = r.src_page_id
             WHERE r.target_key IN ({ph}) AND r.kind IN ('page','tag','page_embed')
             ORDER BY p.key, r.src_block_id, r.kind"
        );
        let mut st = self.conn.prepare(&sql)?;
        let rows: Vec<(i64, Option<i64>, String, String)> = st
            .query_map(params_from_iter(keys.iter()), |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
            })?
            .collect::<std::result::Result<_, _>>()?;
        let mut out: Vec<Backlink> = Vec::new();
        let mut seen: std::collections::HashSet<(i64, i64)> = std::collections::HashSet::new();
        for (page_id, block_id, kind, page_name) in rows {
            if Some(page_id) == own {
                continue;
            }
            match block_id {
                None => out.push(Backlink {
                    source_page: page_name,
                    block_path: None,
                    text: String::new(),
                    breadcrumbs: Vec::new(),
                    kind,
                }),
                Some(bid) => {
                    if !seen.insert((page_id, bid)) {
                        continue;
                    }
                    let (text, path, parent): (String, String, Option<i64>) = self.conn.query_row(
                        "SELECT text, path, parent_id FROM blocks WHERE id = ?1",
                        [bid],
                        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                    )?;
                    let mut crumbs = Vec::new();
                    let mut cur = parent;
                    while let Some(pid) = cur {
                        let (t, pp): (String, Option<i64>) = self.conn.query_row(
                            "SELECT text, parent_id FROM blocks WHERE id = ?1",
                            [pid],
                            |r| Ok((r.get(0)?, r.get(1)?)),
                        )?;
                        crumbs.push(t.lines().next().unwrap_or("").to_string());
                        cur = pp;
                    }
                    crumbs.reverse();
                    out.push(Backlink {
                        source_page: page_name,
                        block_path: Some(path),
                        text,
                        breadcrumbs: crumbs,
                        kind,
                    });
                }
            }
        }
        Ok(out)
    }

    /// Blocks that mention `name` (or one of its aliases) as a whole word outside markup, and don't link to the
    /// page. FTS finds the candidates, `find_mention` checks the word boundaries and the markup.
    pub fn unlinked_refs(&self, name: &str, limit: usize) -> Result<Vec<Unlinked>> {
        let keys = self.keys_for(name);
        let own = self.resolve_page_id(keys.first().map_or("", String::as_str))?;
        let display: String = match own {
            Some(id) => self
                .conn
                .query_row("SELECT name FROM pages WHERE id = ?1", [id], |r| r.get(0))?,
            None => name.to_string(),
        };
        // the page name first, then its aliases (stored folded)
        let mut terms = vec![display];
        terms.extend(keys.iter().skip(1).cloned());
        let ph = vec!["?"; keys.len()].join(",");
        let linked_sql =
            format!("SELECT 1 FROM refs WHERE src_block_id = ? AND target_key IN ({ph}) LIMIT 1");
        let mut out: Vec<Unlinked> = Vec::new();
        let mut seen: std::collections::HashSet<i64> = std::collections::HashSet::new();
        for term in terms {
            let Some(m) = fts_match(&term) else { continue };
            let mut st = self.conn.prepare_cached(
                "SELECT rowid FROM fts WHERE fts MATCH ?1 AND rowid > 0 LIMIT 1000",
            )?;
            let ids: Vec<i64> = st
                .query_map([m], |r| r.get(0))?
                .collect::<std::result::Result<_, _>>()?;
            for bid in ids {
                if out.len() >= limit {
                    return Ok(out);
                }
                if !seen.insert(bid) {
                    continue;
                }
                let (text, path, page_id, parent, page): (
                    String,
                    String,
                    i64,
                    Option<i64>,
                    String,
                ) = self.conn.query_row(
                    "SELECT b.text, b.path, b.page_id, b.parent_id, p.name FROM blocks b
                         JOIN pages p ON p.id = b.page_id WHERE b.id = ?1",
                    [bid],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
                )?;
                if Some(page_id) == own
                    || notatnik_core::mentions::find_mention(&text, &term).is_none()
                {
                    continue;
                }
                let mut args: Vec<rusqlite::types::Value> = vec![bid.into()];
                args.extend(keys.iter().map(|k| k.clone().into()));
                if self
                    .conn
                    .query_row(&linked_sql, params_from_iter(args.iter()), |_| Ok(()))
                    .is_ok()
                {
                    continue; // already linked: it is in the linked references
                }
                out.push(Unlinked {
                    source_page: page,
                    block_path: path,
                    breadcrumbs: self.breadcrumbs(parent)?,
                    term: term.clone(),
                });
            }
        }
        out.sort_by(|a, b| {
            a.source_page
                .to_lowercase()
                .cmp(&b.source_page.to_lowercase())
                .then(a.block_path.cmp(&b.block_path))
        });
        Ok(out)
    }

    /// First lines of the ancestors of a block, outermost first.
    fn breadcrumbs(&self, mut cur: Option<i64>) -> Result<Vec<String>> {
        let mut crumbs = Vec::new();
        while let Some(pid) = cur {
            let (t, pp): (String, Option<i64>) = self.conn.query_row(
                "SELECT text, parent_id FROM blocks WHERE id = ?1",
                [pid],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?;
            crumbs.push(t.lines().next().unwrap_or("").to_string());
            cur = pp;
        }
        crumbs.reverse();
        Ok(crumbs)
    }

    /// Full-text search (case- and diacritic-insensitive; trailing `*` = prefix). Page-name hits come
    /// first, then block hits, each ordered by bm25 relevance.
    pub fn search(&self, q: &str, limit: usize) -> Result<Vec<SearchHit>> {
        let parsed = match crate::searchq::parse(q) {
            Ok(Some(p)) => p,
            Ok(None) => return Ok(Vec::new()),
            Err(m) => return Err(crate::Error::Query(m)),
        };
        let m = parsed.fts.clone();
        let mut st = self.conn.prepare_cached("SELECT rowid, bm25(fts, 10.0, 1.0) FROM fts WHERE fts MATCH ?1 ORDER BY bm25(fts, 10.0, 1.0) LIMIT ?2")?;
        let rows: Vec<(i64, f64)> = st
            .query_map(params![m, (limit * 3).max(30) as i64], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })?
            .collect::<std::result::Result<_, _>>()?;
        let mut hits = Vec::new();
        for (rowid, score) in rows {
            if rowid < 0 {
                let name: String =
                    self.conn
                        .query_row("SELECT name FROM pages WHERE id = ?1", [-rowid], |r| {
                            r.get(0)
                        })?;
                hits.push(SearchHit {
                    kind: SearchKind::Page,
                    highlights: highlights(&name, &parsed.positive),
                    page: name.clone(),
                    path: None,
                    text: name,
                    score,
                });
            } else {
                let (text, path, page): (String, String, String) = self.conn.query_row(
                    "SELECT b.text, b.path, p.name FROM blocks b JOIN pages p ON p.id = b.page_id WHERE b.id = ?1",
                    [rowid],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                )?;
                hits.push(SearchHit {
                    kind: SearchKind::Block,
                    highlights: highlights(&text, &parsed.positive),
                    page,
                    path: Some(path),
                    text,
                    score,
                });
            }
        }
        hits.sort_by(|a, b| {
            (a.kind == SearchKind::Block)
                .cmp(&(b.kind == SearchKind::Block))
                .then(
                    a.score
                        .partial_cmp(&b.score)
                        .unwrap_or(std::cmp::Ordering::Equal),
                )
        });
        hits.truncate(limit);
        Ok(hits)
    }

    /// The files of the pages that have a block with `needle` in its text, e.g. the name of a drawing (DR-14): these
    /// are indexed again when the drawing they show changes.
    pub fn files_mentioning(&self, needle: &str) -> Result<Vec<String>> {
        let like = format!(
            "%{}%",
            needle
                .replace('\\', "\\\\")
                .replace('%', "\\%")
                .replace('_', "\\_")
        );
        let mut st = self.conn.prepare(
            "SELECT DISTINCT p.file_path FROM blocks b JOIN pages p ON p.id = b.page_id \
             WHERE b.text LIKE ?1 ESCAPE '\\' AND p.file_path IS NOT NULL",
        )?;
        let rows = st.query_map([like], |r| r.get::<_, String>(0))?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    /// All recorded issues, ordered by path and offset.
    pub fn issues(&self) -> Result<Vec<IssueRow>> {
        let mut st = self.conn.prepare(
            "SELECT path, severity, code, message, offset FROM issues ORDER BY path, offset, code",
        )?;
        let rows = st
            .query_map([], |r| {
                Ok(IssueRow {
                    path: r.get(0)?,
                    severity: r.get(1)?,
                    code: r.get(2)?,
                    message: r.get(3)?,
                    offset: r.get(4)?,
                })
            })?
            .collect::<std::result::Result<_, _>>()?;
        Ok(rows)
    }

    /// Page links (not tags) that point at pages without a file: `(source page, target name)`.
    pub fn unresolved_links(&self) -> Result<Vec<(String, String)>> {
        let mut st = self.conn.prepare(
            "SELECT DISTINCT sp.name, t.name FROM refs r
             JOIN pages sp ON sp.id = r.src_page_id
             JOIN pages t ON t.key = r.target_key
             WHERE r.kind IN ('page','page_embed') AND t.is_virtual = 1
               AND NOT EXISTS (SELECT 1 FROM aliases a WHERE a.alias_key = r.target_key)
               AND t.is_journal = 0
             ORDER BY sp.key, t.key",
        )?;
        let rows = st
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<std::result::Result<_, _>>()?;
        Ok(rows)
    }

    /// Block references `((id))` whose target ID doesn't exist: `(source page, block path, id)`.
    pub fn dangling_block_refs(&self) -> Result<Vec<(String, String, String)>> {
        let mut st = self.conn.prepare(
            "SELECT p.name, b.path, r.target_uid FROM refs r
             JOIN pages p ON p.id = r.src_page_id JOIN blocks b ON b.id = r.src_block_id
             WHERE r.kind IN ('block','block_embed')
               AND NOT EXISTS (SELECT 1 FROM blocks t WHERE t.uid = r.target_uid)
             ORDER BY p.key, b.id",
        )?;
        let rows = st
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<std::result::Result<_, _>>()?;
        Ok(rows)
    }

    /// Looks up a block by its `^id`: `(page name, block path, text)`.
    pub fn block_by_uid(&self, uid: &str) -> Result<Option<(String, String, String)>> {
        Ok(self
            .conn
            .query_row(
                "SELECT p.name, b.path, b.text FROM blocks b JOIN pages p ON p.id = b.page_id WHERE b.uid = ?1",
                [uid],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?)
    }

    /// Uids that some block's `follows::` points to (the tasks that have a follow-up).
    pub fn followed_uids(&self) -> Result<std::collections::HashSet<String>> {
        let mut st = self.conn.prepare(
            "SELECT DISTINCT value_text FROM props WHERE owner_kind = 'block' AND key = 'follows'",
        )?;
        let rows: Vec<String> = st
            .query_map([], |r| r.get::<_, Option<String>>(0))?
            .filter_map(|v| v.ok().flatten())
            .collect();
        Ok(rows.iter().filter_map(|v| block_ref_uid(v)).collect())
    }

    /// The chain of tasks a block is in (TK-15): everything it follows, back to the first task, and everything that
    /// follows it, in order (a task with several follow-ups is a branch; `depth` is the nesting). `at` is the
    /// block's page name and path (`0.2`). `None` when the block is not in the index.
    pub fn task_chain(&self, page: &str, path: &str) -> Result<Option<(Vec<ChainTask>, usize)>> {
        let Some(start) = self
            .conn
            .query_row(
                "SELECT b.id FROM blocks b JOIN pages p ON p.id = b.page_id WHERE p.name = ?1 AND b.path = ?2",
                [page, path],
                |r| r.get::<_, i64>(0),
            )
            .optional()?
        else {
            return Ok(None);
        };
        // back to the first task (a cycle or a missing target ends the walk)
        let mut seen = std::collections::HashSet::from([start]);
        let mut root = start;
        let mut missing: Option<String> = None;
        for _ in 0..500 {
            let Some(prev) = self.follows_of(root)? else {
                break;
            };
            match self.block_with_uid(&prev)? {
                Some(id) if seen.insert(id) => root = id,
                Some(_) => break,
                None => {
                    missing = Some(prev);
                    break;
                }
            }
        }
        let mut out = Vec::new();
        if let Some(uid) = missing {
            out.push(ChainTask {
                uid: Some(uid),
                missing: true,
                ..ChainTask::default()
            });
        }
        let depth0 = usize::from(!out.is_empty());
        let mut visited = std::collections::HashSet::new();
        let mut stack = vec![(root, depth0)];
        let mut position = 0;
        while let Some((id, depth)) = stack.pop() {
            if !visited.insert(id) || out.len() > 500 {
                continue;
            }
            if id == start {
                position = out.len();
            }
            let mut t = self.chain_task(id)?;
            t.depth = depth;
            let uid = t.uid.clone();
            out.push(t);
            if let Some(uid) = uid {
                // later followers are pushed first so that they come out in file order
                let mut next = self.followers_of(&uid)?;
                next.reverse();
                stack.extend(next.into_iter().map(|n| (n, depth + 1)));
            }
        }
        Ok(Some((out, position)))
    }

    fn follows_of(&self, block_id: i64) -> Result<Option<String>> {
        let v: Option<String> = self
            .conn
            .query_row(
                "SELECT value_text FROM props WHERE owner_kind = 'block' AND owner_id = ?1 AND key = 'follows' ORDER BY ord LIMIT 1",
                [block_id],
                |r| r.get(0),
            )
            .optional()?
            .flatten();
        Ok(v.and_then(|v| block_ref_uid(&v)))
    }

    fn block_with_uid(&self, uid: &str) -> Result<Option<i64>> {
        Ok(self
            .conn
            .query_row("SELECT id FROM blocks WHERE uid = ?1", [uid], |r| r.get(0))
            .optional()?)
    }

    fn followers_of(&self, uid: &str) -> Result<Vec<i64>> {
        let mut st = self.conn.prepare(
            "SELECT b.id FROM props pr JOIN blocks b ON b.id = pr.owner_id JOIN pages p ON p.id = b.page_id
             WHERE pr.owner_kind = 'block' AND pr.key = 'follows' AND pr.value_text = ?1
             ORDER BY p.name, b.id",
        )?;
        let rows = st
            .query_map([format!("(({uid}))")], |r| r.get::<_, i64>(0))?
            .collect::<std::result::Result<_, _>>()?;
        Ok(rows)
    }

    fn chain_task(&self, id: i64) -> Result<ChainTask> {
        Ok(self.conn.query_row(
            "SELECT b.uid, p.name, b.path, b.text, b.task_state, b.scheduled, b.deadline
             FROM blocks b JOIN pages p ON p.id = b.page_id WHERE b.id = ?1",
            [id],
            |r| {
                Ok(ChainTask {
                    uid: r.get(0)?,
                    page: r.get(1)?,
                    path: r.get(2)?,
                    text: r.get(3)?,
                    task_state: r.get(4)?,
                    scheduled: r.get(5)?,
                    deadline: r.get(6)?,
                    depth: 0,
                    missing: false,
                })
            },
        )?)
    }

    /// All block IDs in the index (for generating collision-free IDs).
    pub fn all_block_uids(&self) -> Result<std::collections::HashSet<String>> {
        let mut st = self
            .conn
            .prepare("SELECT uid FROM blocks WHERE uid IS NOT NULL")?;
        let rows = st
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<std::result::Result<_, _>>()?;
        Ok(rows)
    }

    /// Page names and file paths of all pages that have a file.
    pub fn page_files(&self) -> Result<Vec<(String, String)>> {
        let mut st = self.conn.prepare(
            "SELECT name, file_path FROM pages WHERE file_path IS NOT NULL ORDER BY key",
        )?;
        let rows = st
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<std::result::Result<_, _>>()?;
        Ok(rows)
    }

    /// All pages that reference `name` anywhere (used by rename): paths of files that contain refs to it.
    pub fn files_referencing(&self, name: &str) -> Result<Vec<String>> {
        let keys = self.keys_for(name);
        let ph = vec!["?"; keys.len()].join(",");
        let sql = format!(
            "SELECT DISTINCT p.file_path FROM refs r JOIN pages p ON p.id = r.src_page_id
             WHERE r.target_key IN ({ph}) AND r.kind IN ('page','tag','page_embed') AND p.file_path IS NOT NULL ORDER BY p.file_path"
        );
        let mut st = self.conn.prepare(&sql)?;
        let rows = st
            .query_map(
                params_from_iter(keys.iter().map(|k| Value::Text(k.clone()))),
                |r| r.get(0),
            )?
            .collect::<std::result::Result<_, _>>()?;
        Ok(rows)
    }

    /// Page-property keys (frontmatter) with the number of pages that have them, most used first.
    pub fn page_property_keys(&self) -> Result<Vec<PageKeyCount>> {
        let mut st = self.conn.prepare_cached(
            "SELECT pr.key, COUNT(DISTINCT pr.owner_id) AS n FROM props pr
             JOIN pages p ON p.id = pr.owner_id
             WHERE pr.owner_kind = 'page' AND p.is_journal = 0
             GROUP BY pr.key ORDER BY n DESC, pr.key",
        )?;
        let rows = st
            .query_map([], |r| {
                Ok(PageKeyCount {
                    key: r.get(0)?,
                    pages: r.get::<_, i64>(1)? as usize,
                })
            })?
            .collect::<std::result::Result<_, _>>()?;
        Ok(rows)
    }

    /// Pages grouped by the value of a page property. A page with several values (a list) appears in each group;
    /// values that differ only in case or accents are one group. Pages without the property come last in a group
    /// whose `value` is `None`. Journals and pages without a file are not listed.
    pub fn page_groups(&self, key: &str) -> Result<Vec<PageGroup>> {
        let mut st = self.conn.prepare_cached(
            "SELECT pr.value_fold, pr.value_text, p.name FROM props pr JOIN pages p ON p.id = pr.owner_id
             WHERE pr.owner_kind = 'page' AND pr.key = ?1 AND p.is_journal = 0 AND p.is_virtual = 0
               AND pr.value_text IS NOT NULL AND pr.value_text <> ''
             ORDER BY pr.value_fold, p.key",
        )?;
        let rows: Vec<(String, String, String)> = st
            .query_map([key], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<std::result::Result<_, _>>()?;
        let mut groups: Vec<(String, PageGroup)> = Vec::new();
        for (fold, text, page) in rows {
            match groups.last_mut() {
                Some((f, g)) if *f == fold => {
                    if !g.pages.contains(&page) {
                        g.pages.push(page);
                    }
                }
                _ => groups.push((
                    fold,
                    PageGroup {
                        value: Some(text),
                        pages: vec![page],
                    },
                )),
            }
        }
        let mut out: Vec<PageGroup> = groups.into_iter().map(|(_, g)| g).collect();
        let mut st = self.conn.prepare_cached(
            "SELECT p.name FROM pages p WHERE p.is_journal = 0 AND p.is_virtual = 0
               AND p.id NOT IN (SELECT owner_id FROM props WHERE owner_kind = 'page' AND key = ?1
                                AND value_text IS NOT NULL AND value_text <> '')
             ORDER BY p.key",
        )?;
        let none: Vec<String> = st
            .query_map([key], |r| r.get(0))?
            .collect::<std::result::Result<_, _>>()?;
        if !none.is_empty() {
            out.push(PageGroup {
                value: None,
                pages: none,
            });
        }
        Ok(out)
    }

    /// Pages whose page property `key` has `value` (no regard to case or accents). Pages without a file are skipped.
    pub fn pages_with_property(&self, key: &str, value: &str) -> Result<Vec<String>> {
        let mut st = self.conn.prepare_cached(
            "SELECT DISTINCT p.name FROM props pr JOIN pages p ON p.id = pr.owner_id
             WHERE pr.owner_kind = 'page' AND pr.key = ?1 AND pr.value_fold = ?2 AND p.is_virtual = 0
             ORDER BY p.key",
        )?;
        let rows = st
            .query_map(params![key, search_fold(value)], |r| r.get(0))?
            .collect::<std::result::Result<_, _>>()?;
        Ok(rows)
    }

    /// The blocks that have the property `key` with this value: `(page name, block ID if it has one)`. The value
    /// is compared the way searches are (case and accents ignored).
    pub fn blocks_with_property(
        &self,
        key: &str,
        value: &str,
    ) -> Result<Vec<(String, Option<String>)>> {
        let mut st = self.conn.prepare_cached(
            "SELECT p.name, b.uid FROM props pr
             JOIN blocks b ON b.id = pr.owner_id JOIN pages p ON p.id = b.page_id
             WHERE pr.owner_kind = 'block' AND pr.key = ?1 AND pr.value_fold = ?2 AND p.is_virtual = 0
             ORDER BY p.key, b.id",
        )?;
        let rows = st
            .query_map(params![key, search_fold(value)], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })?
            .collect::<std::result::Result<_, _>>()?;
        Ok(rows)
    }

    /// Name of the page whose file is `rel` (relative path).
    pub fn page_for_file(&self, rel: &str) -> Result<Option<String>> {
        let mut st = self.conn.prepare_cached(
            "SELECT p.name FROM files f JOIN pages p ON p.id = f.page_id WHERE f.path = ?1",
        )?;
        let mut rows = st.query_map([rel], |r| r.get::<_, String>(0))?;
        Ok(rows.next().transpose()?)
    }

    /// Pages whose frontmatter says `archived: true` (also `yes`, `1`). Pages below an archived page count as
    /// archived too; that is the caller's rule, this lists only the pages that carry the property.
    pub fn archived_pages(&self) -> Result<Vec<String>> {
        let mut st = self.conn.prepare_cached(
            "SELECT DISTINCT p.name FROM props pr JOIN pages p ON p.id = pr.owner_id
             WHERE pr.owner_kind = 'page' AND pr.key = 'archived' AND p.is_virtual = 0
               AND lower(trim(pr.value_text)) IN ('true', 'yes', '1', 'y')
             ORDER BY p.key",
        )?;
        let rows = st
            .query_map([], |r| r.get(0))?
            .collect::<std::result::Result<_, _>>()?;
        Ok(rows)
    }

    /// `(page, value)` for every page that has the page property `key` (one row per value of a list).
    pub fn page_property_values(&self, key: &str) -> Result<Vec<(String, String)>> {
        let mut st = self.conn.prepare_cached(
            "SELECT p.name, pr.value_text FROM props pr JOIN pages p ON p.id = pr.owner_id
             WHERE pr.owner_kind = 'page' AND pr.key = ?1 AND p.is_virtual = 0
               AND pr.value_text IS NOT NULL AND pr.value_text <> ''
             ORDER BY p.key, pr.ord",
        )?;
        let rows = st
            .query_map([key], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<std::result::Result<_, _>>()?;
        Ok(rows)
    }

    /// Every place where `segment` (a name, or `a/b` for two segments in a row) occurs in a page path, with the pages
    /// below it: `Renewal` finds `P/Capgemini/Renewal` (and its `Siemens Energy`), `P/EY/Renewal`, `X/Renewal`.
    /// Matching ignores case. Journals are skipped. Ordered by the path.
    pub fn segment_groups(&self, segment: &str) -> Result<Vec<SegmentGroup>> {
        let Ok(n) = PageName::parse(segment) else {
            return Ok(Vec::new());
        };
        let seg_key = n.lookup_key();
        let seg: Vec<&str> = segs_of(&seg_key);
        let mut st = self.conn.prepare_cached(
            "SELECT name, key, is_virtual FROM pages WHERE is_journal = 0 ORDER BY key",
        )?;
        let rows: Vec<(String, String, bool)> = st
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get::<_, i64>(2)? != 0)))?
            .collect::<std::result::Result<_, _>>()?;
        // group by the lookup key of the occurrence; the first page seen names the display form
        let mut groups: Vec<(String, SegmentGroup)> = Vec::new();
        let mut heads: std::collections::HashMap<String, bool> = std::collections::HashMap::new();
        for (name, key, is_virtual) in &rows {
            let ks = segs_of(key);
            let Some(end) = occurrence_end(&ks, &seg) else {
                continue;
            };
            let occ_key = ks[..end].join("/");
            if end == ks.len() {
                heads.insert(occ_key.clone(), *is_virtual);
            }
            let occ_name = name.split('/').take(end).collect::<Vec<_>>().join("/");
            // a top-level placeholder (only referenced, e.g. by a `#renewal` tag) with nothing below it is the tag, not a
            // place; a page inside a path (`Napis/first/deepest`, linked but never written) belongs to the hierarchy
            if ks.len() == 1
                && end == ks.len()
                && *is_virtual
                && !rows
                    .iter()
                    .any(|(_, k, _)| k.starts_with(&format!("{key}/")))
            {
                continue;
            }
            let idx = match groups.iter().position(|(k, _)| *k == occ_key) {
                Some(i) => i,
                None => {
                    groups.push((
                        occ_key.clone(),
                        SegmentGroup {
                            occurrence: occ_name,
                            occurrence_is_virtual: true,
                            pages: Vec::new(),
                        },
                    ));
                    groups.len() - 1
                }
            };
            if end < ks.len() {
                groups[idx].1.pages.push((name.clone(), *is_virtual));
            }
        }
        Ok(groups
            .into_iter()
            .map(|(k, mut g)| {
                g.occurrence_is_virtual = heads.get(&k).copied().unwrap_or(true);
                g
            })
            .collect())
    }

    /// The file of each page name (relative to the graph), `None` for a page without a file or an unknown name.
    pub fn files_of_pages(&self, names: &[String]) -> Result<Vec<Option<String>>> {
        let mut st = self
            .conn
            .prepare_cached("SELECT file_path FROM pages WHERE key = ?1 AND is_virtual = 0")?;
        names
            .iter()
            .map(|n| {
                let Ok(n) = PageName::parse(n) else {
                    return Ok(None);
                };
                Ok(st
                    .query_row([n.lookup_key()], |r| r.get::<_, Option<String>>(0))
                    .optional()?
                    .flatten())
            })
            .collect()
    }

    /// For each segment, in how many different places it occurs (see `segment_groups`); one pass over all pages.
    pub fn segment_places(&self, segments: &[String]) -> Result<Vec<usize>> {
        let wanted: Vec<String> = segments
            .iter()
            .map(|s| {
                PageName::parse(s)
                    .map(|n| n.lookup_key())
                    .unwrap_or_default()
            })
            .collect();
        let mut st = self.conn.prepare_cached(
            "SELECT key, is_virtual FROM pages WHERE is_journal = 0 ORDER BY key",
        )?;
        let keys: Vec<(String, bool)> = st
            .query_map([], |r| Ok((r.get(0)?, r.get::<_, i64>(1)? != 0)))?
            .collect::<std::result::Result<_, _>>()?;
        let mut sets: Vec<std::collections::HashSet<String>> =
            vec![Default::default(); wanted.len()];
        for (n, (key, is_virtual)) in keys.iter().enumerate() {
            let ks = segs_of(key);
            // sorted by key, the pages below a page follow it directly
            let has_below = keys
                .get(n + 1)
                .is_some_and(|(next, _)| next.starts_with(&format!("{key}/")));
            for (i, w) in wanted.iter().enumerate() {
                if w.is_empty() {
                    continue;
                }
                if let Some(end) = occurrence_end(&ks, &segs_of(w)) {
                    // a top-level placeholder with nothing below it is not a place (see `segment_groups`)
                    if ks.len() == 1 && end == ks.len() && *is_virtual && !has_below {
                        continue;
                    }
                    sets[i].insert(ks[..end].join("/"));
                }
            }
        }
        Ok(sets.into_iter().map(|s| s.len()).collect())
    }

    /// Every page below `name` in the namespace tree (all depths, real and virtual), by name.
    pub fn subtree_pages(&self, name: &str) -> Result<Vec<SubPage>> {
        let Ok(n) = PageName::parse(name) else {
            return Ok(Vec::new());
        };
        let key = n.lookup_key();
        let mut st = self.conn.prepare(
            "SELECT name, is_virtual, file_path FROM pages
             WHERE key > ?1 || '/' AND key < ?1 || '0' ORDER BY key",
        )?;
        let rows = st
            .query_map([key], |r| {
                Ok(SubPage {
                    name: r.get(0)?,
                    is_virtual: r.get::<_, i64>(1)? != 0,
                    file_path: r.get(2)?,
                })
            })?
            .collect::<std::result::Result<_, _>>()?;
        Ok(rows)
    }

    /// Files that reference `name` (aliases included) or any page below it (used by namespace moves).
    pub fn files_referencing_subtree(&self, name: &str) -> Result<Vec<String>> {
        let keys = self.keys_for(name);
        let own = keys.first().cloned().unwrap_or_default();
        let ph = vec!["?"; keys.len()].join(",");
        let sql = format!(
            "SELECT DISTINCT p.file_path FROM refs r JOIN pages p ON p.id = r.src_page_id
             WHERE (r.target_key IN ({ph}) OR (r.target_key > ? || '/' AND r.target_key < ? || '0'))
               AND r.kind IN ('page','tag','page_embed') AND p.file_path IS NOT NULL ORDER BY p.file_path"
        );
        let mut args: Vec<Value> = keys.iter().map(|k| Value::Text(k.clone())).collect();
        args.push(Value::Text(own.clone()));
        args.push(Value::Text(own));
        let mut st = self.conn.prepare(&sql)?;
        let rows = st
            .query_map(params_from_iter(args.iter()), |r| r.get(0))?
            .collect::<std::result::Result<_, _>>()?;
        Ok(rows)
    }

    /// Direct child pages in the namespace tree (names), real and virtual.
    pub fn namespace_children(&self, name: &str) -> Result<Vec<String>> {
        let Ok(n) = PageName::parse(name) else {
            return Ok(Vec::new());
        };
        let mut st = self.conn.prepare("SELECT c.name FROM pages c JOIN pages p ON c.ns_parent_id = p.id WHERE p.key = ?1 ORDER BY c.key")?;
        let rows = st
            .query_map([n.lookup_key()], |r| r.get(0))?
            .collect::<std::result::Result<_, _>>()?;
        Ok(rows)
    }
}

impl Index {
    /// The content hash (blake3 hex) stored for a file, if it is indexed.
    pub fn file_hash(&self, rel: &str) -> Result<Option<String>> {
        Ok(self
            .conn
            .query_row("SELECT hash FROM files WHERE path = ?1", [rel], |r| {
                r.get(0)
            })
            .optional()?)
    }
}

/// Pages that share one value of a page property (`type: person`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PageGroup {
    /// The value as first written; `None` for the pages that don't have the property.
    pub value: Option<String>,
    /// Display names of the pages, ordered by name.
    pub pages: Vec<String>,
}

/// A page-property key with the number of pages that have it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PageKeyCount {
    /// Property key.
    pub key: String,
    /// Pages that have it.
    pub pages: usize,
}

/// One place where a path segment occurs: the page that ends in it and the pages below that page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SegmentGroup {
    /// The path up to and including the segment (`P/EY/Renewal`).
    pub occurrence: String,
    /// The occurrence has no file of its own (it only exists as the parent of other pages).
    pub occurrence_is_virtual: bool,
    /// The pages below the occurrence, by name (`(name, is_virtual)`).
    pub pages: Vec<(String, bool)>,
}

/// Where a segment of a page name (or several segments in a row) ends: the number of segments up to and including its
/// first occurrence in `key_segments`, if any.
fn occurrence_end(key_segments: &[&str], seg: &[&str]) -> Option<usize> {
    if seg.is_empty() || key_segments.len() < seg.len() {
        return None;
    }
    (0..=key_segments.len() - seg.len())
        .find(|i| key_segments[*i..*i + seg.len()] == *seg)
        .map(|i| i + seg.len())
}

/// A page name as lookup-key segments (case and Unicode form folded).
fn segs_of(key: &str) -> Vec<&str> {
    key.split('/').collect()
}

/// A page below another one in the namespace tree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SubPage {
    /// Display name.
    pub name: String,
    /// No file behind it.
    pub is_virtual: bool,
    /// Path of its file relative to the graph root.
    pub file_path: Option<String>,
}

/// One row of the "all pages" list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PageRow {
    /// Display name.
    pub name: String,
    /// No file behind it.
    pub is_virtual: bool,
    /// Name is a date.
    pub is_journal: bool,
    /// Blocks in the page (without the preamble).
    pub blocks: usize,
    /// Blocks and pages elsewhere that link to it (page links, tags, embeds).
    pub backlinks: usize,
    /// File modification time, nanoseconds since the Unix epoch.
    pub mtime_ns: Option<i64>,
    /// The file, relative to the graph.
    pub file_path: Option<String>,
}

/// One entry of the namespace tree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NamespaceRow {
    /// Full page name.
    pub name: String,
    /// No file behind it.
    pub is_virtual: bool,
    /// Number of pages below it, at any depth.
    pub descendants: usize,
    /// The file, relative to the graph.
    pub file_path: Option<String>,
}

impl Index {
    /// Direct children of `parent` in the namespace tree (`None` = top level, journals excluded),
    /// with the number of pages below each.
    pub fn namespace_entries(&self, parent: Option<&str>) -> Result<Vec<NamespaceRow>> {
        let parent_id: Option<i64> = match parent {
            Some(p) => {
                let Ok(n) = PageName::parse(p) else {
                    return Ok(Vec::new());
                };
                match self.resolve_page_id(&n.lookup_key())? {
                    Some(id) => Some(id),
                    None => return Ok(Vec::new()),
                }
            }
            None => None,
        };
        let sql = "SELECT c.name, c.is_virtual,
                          (SELECT COUNT(*) FROM pages d WHERE d.key > c.key || '/' AND d.key < c.key || '0'),
                          c.file_path
                   FROM pages c
                   WHERE c.ns_parent_id IS ?1 AND c.is_journal = 0
                   ORDER BY c.key";
        let mut st = self.conn.prepare_cached(sql)?;
        let rows = st
            .query_map([parent_id], |r| {
                Ok(NamespaceRow {
                    name: r.get(0)?,
                    is_virtual: r.get::<_, i64>(1)? != 0,
                    descendants: r.get::<_, i64>(2)? as usize,
                    file_path: r.get(3)?,
                })
            })?
            .collect::<std::result::Result<_, _>>()?;
        Ok(rows)
    }

    /// Every page (files and referenced-only pages) with a few counts, ordered by name.
    pub fn all_pages(&self) -> Result<Vec<PageRow>> {
        // grouped once instead of one subquery per page (10,000 pages: 4 s -> tens of ms)
        let sql = "WITH bc AS (SELECT page_id, COUNT(*) n FROM blocks WHERE is_preamble = 0 GROUP BY page_id),
                        bl AS (SELECT p2.id AS pid, COUNT(*) n
                               FROM (SELECT DISTINCT src_page_id, COALESCE(src_block_id, 0) AS b, target_key
                                     FROM refs WHERE kind IN ('page','tag','page_embed')) r
                               JOIN pages p2 ON p2.key = r.target_key
                               WHERE r.src_page_id <> p2.id GROUP BY p2.id)
                   SELECT p.name, p.is_virtual, p.is_journal, COALESCE(bc.n, 0), COALESCE(bl.n, 0), f.mtime_ns, p.file_path
                   FROM pages p
                   LEFT JOIN bc ON bc.page_id = p.id
                   LEFT JOIN bl ON bl.pid = p.id
                   LEFT JOIN files f ON f.path = p.file_path
                   ORDER BY p.key";
        let mut st = self.conn.prepare_cached(sql)?;
        let rows = st
            .query_map([], |r| {
                Ok(PageRow {
                    name: r.get(0)?,
                    is_virtual: r.get::<_, i64>(1)? != 0,
                    is_journal: r.get::<_, i64>(2)? != 0,
                    blocks: r.get::<_, i64>(3)? as usize,
                    backlinks: r.get::<_, i64>(4)? as usize,
                    mtime_ns: r.get(5)?,
                    file_path: r.get(6)?,
                })
            })?
            .collect::<std::result::Result<_, _>>()?;
        Ok(rows)
    }

    /// Whether a page with this name exists as a file.
    pub fn page_has_file(&self, name: &str) -> Result<bool> {
        Ok(self.page_info(name)?.is_some_and(|p| !p.is_virtual))
    }
}

impl Index {
    /// Block property keys, most used first.
    pub fn property_keys(&self, limit: usize) -> Result<Vec<String>> {
        let mut st = self.conn.prepare_cached("SELECT key FROM props WHERE owner_kind = 'block' GROUP BY key ORDER BY COUNT(*) DESC, key LIMIT ?1")?;
        let rows = st
            .query_map([limit as i64], |r| r.get(0))?
            .collect::<std::result::Result<_, _>>()?;
        Ok(rows)
    }

    /// Values seen for a block property key, most used first.
    pub fn property_values(&self, key: &str, limit: usize) -> Result<Vec<String>> {
        let mut st = self.conn.prepare_cached(
            "SELECT value_text FROM props WHERE owner_kind = 'block' AND key = ?1 AND value_text IS NOT NULL GROUP BY value_text ORDER BY COUNT(*) DESC, value_text LIMIT ?2",
        )?;
        let rows = st
            .query_map(params![key, limit as i64], |r| r.get(0))?
            .collect::<std::result::Result<_, _>>()?;
        Ok(rows)
    }
}
