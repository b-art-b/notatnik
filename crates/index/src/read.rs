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

fn highlights(text: &str, q: &str) -> Vec<(usize, usize)> {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let folded: Vec<char> = chars.iter().map(|(_, c)| fold_char(*c)).collect();
    let mut out: Vec<(usize, usize)> = Vec::new();
    for (tok, prefix) in match_tokens(q) {
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

    /// Full-text search (case- and diacritic-insensitive; trailing `*` = prefix). Page-name hits come
    /// first, then block hits, each ordered by bm25 relevance.
    pub fn search(&self, q: &str, limit: usize) -> Result<Vec<SearchHit>> {
        let Some(m) = fts_match(q) else {
            return Ok(Vec::new());
        };
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
                    highlights: highlights(&name, q),
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
                    highlights: highlights(&text, q),
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
