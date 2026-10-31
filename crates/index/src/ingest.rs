//! Writing files into the index: full index, incremental reindex, removal, startup reconcile.

use std::collections::{BTreeMap, HashSet, VecDeque};
use std::path::Path;
use std::time::UNIX_EPOCH;

use notatnik_core::{
    Block, Page, PageName, ParseOptions, PropertyValue, RefKind, Severity,
    props::split_top_level_commas, search_fold,
};
use rayon::prelude::*;
use rusqlite::{OptionalExtension, Transaction, params};

use crate::{Error, Index, Result, walk};

/// File metadata used to detect changes cheaply.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileStat {
    /// Modification time in nanoseconds since the epoch.
    pub mtime_ns: i64,
    /// Size in bytes.
    pub size: i64,
}

/// A block whose ID was already used by another page; the caller should write a new ID to the file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateId {
    /// Child-index path of the block in its file.
    pub path: Vec<usize>,
    /// The duplicated ID.
    pub id: String,
}

/// What a reindex did.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ReindexOutcome {
    /// Page name (empty if the file was removed or not indexable).
    pub page: String,
    /// The file was removed from the index.
    pub removed: bool,
    /// Blocks with duplicate IDs that need a new ID in the file.
    pub duplicates: Vec<DuplicateId>,
}

/// Counters of a startup reconcile (P1.33).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ReconcileStats {
    /// Files that were parsed and written (changed or new).
    pub reindexed: usize,
    /// Of those, new files.
    pub added: usize,
    /// Files removed because they are gone from disk.
    pub deleted: usize,
    /// Files whose mtime changed but whose content hash did not (only the stat was updated).
    pub touched: usize,
    /// Files whose content was hashed.
    pub hashed: usize,
    /// Duplicate block IDs found (the caller rewrites them).
    pub duplicates: Vec<(String, Vec<DuplicateId>)>,
    /// Paths that were (re)indexed.
    pub reindexed_paths: Vec<String>,
    /// Paths that were removed from the index.
    pub deleted_paths: Vec<String>,
}

pub(crate) struct Parsed {
    rel: String,
    name: PageName,
    text: String,
    page: Page,
    hash: String,
    stat: FileStat,
}

pub(crate) fn stat_of(path: &Path) -> std::io::Result<FileStat> {
    let md = std::fs::metadata(path)?;
    let mtime_ns = md
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| i64::try_from(d.as_nanos()).unwrap_or(i64::MAX));
    Ok(FileStat {
        mtime_ns,
        size: i64::try_from(md.len()).unwrap_or(i64::MAX),
    })
}

enum ReadResult {
    Ok(Box<Parsed>),
    Skip {
        rel: String,
        code: &'static str,
        message: String,
    },
}

fn read_parse(root: &Path, rel: &str, opts: &ParseOptions) -> ReadResult {
    let abs = root.join(rel);
    let bytes = match std::fs::read(&abs) {
        Ok(b) => b,
        Err(e) => {
            return ReadResult::Skip {
                rel: rel.into(),
                code: "unreadable",
                message: format!("cannot read file: {e}"),
            };
        }
    };
    let stat = stat_of(&abs).unwrap_or(FileStat {
        mtime_ns: 0,
        size: bytes.len() as i64,
    });
    let hash = blake3::hash(&bytes).to_hex().to_string();
    let Ok(text) = String::from_utf8(bytes) else {
        return ReadResult::Skip {
            rel: rel.into(),
            code: "not-utf8",
            message: "file is not valid UTF-8 and was skipped".into(),
        };
    };
    let Some(name) = notatnik_core::path_to_page(rel) else {
        return ReadResult::Skip {
            rel: rel.into(),
            code: "unmapped",
            message: "file does not map to a page name".into(),
        };
    };
    let page = notatnik_core::parse_page_with(&text, opts);
    ReadResult::Ok(Box::new(Parsed {
        rel: rel.into(),
        name,
        text,
        page,
        hash,
        stat,
    }))
}

fn key_of(name: &str) -> String {
    PageName::parse(name).map_or_else(|_| search_fold(name), |n| n.lookup_key())
}

fn ensure_page(tx: &Transaction, name: &PageName) -> Result<i64> {
    let key = name.lookup_key();
    if let Some(id) = tx
        .query_row("SELECT id FROM pages WHERE key = ?1", [&key], |r| {
            r.get::<_, i64>(0)
        })
        .optional()?
    {
        return Ok(id);
    }
    let parent = match name.parent() {
        Some(p) => Some(ensure_page(tx, &p)?),
        None => None,
    };
    let jd = name
        .journal_date()
        .map(|d| d.format("%Y-%m-%d").to_string());
    tx.execute(
        "INSERT INTO pages(name, key, ns_parent_id, is_journal, journal_date, is_virtual) VALUES (?1, ?2, ?3, ?4, ?5, 1)",
        params![name.as_str(), key, parent, i64::from(jd.is_some()), jd],
    )?;
    Ok(tx.last_insert_rowid())
}

fn clear_page_content(tx: &Transaction, page_id: i64) -> Result<()> {
    tx.execute(
        "DELETE FROM fts WHERE rowid IN (SELECT id FROM blocks WHERE page_id = ?1)",
        [page_id],
    )?;
    tx.execute("DELETE FROM fts WHERE rowid = ?1", [-page_id])?;
    tx.execute("DELETE FROM props WHERE owner_kind = 'block' AND owner_id IN (SELECT id FROM blocks WHERE page_id = ?1)", [page_id])?;
    tx.execute(
        "DELETE FROM props WHERE owner_kind = 'page' AND owner_id = ?1",
        [page_id],
    )?;
    tx.execute("DELETE FROM refs WHERE src_page_id = ?1", [page_id])?;
    tx.execute("DELETE FROM blocks WHERE page_id = ?1", [page_id])?;
    tx.execute("DELETE FROM aliases WHERE page_id = ?1", [page_id])?;
    Ok(())
}

fn old_keys(tx: &Transaction, page_id: i64) -> Result<HashSet<String>> {
    let mut keys = HashSet::new();
    let mut st = tx.prepare_cached(
        "SELECT DISTINCT target_key FROM refs WHERE src_page_id = ?1 AND target_key IS NOT NULL",
    )?;
    for k in st.query_map([page_id], |r| r.get::<_, String>(0))? {
        keys.insert(k?);
    }
    let mut st = tx.prepare_cached("SELECT alias_key FROM aliases WHERE page_id = ?1")?;
    for k in st.query_map([page_id], |r| r.get::<_, String>(0))? {
        keys.insert(k?);
    }
    Ok(keys)
}

/// Deletes virtual pages that nothing needs any more (no children, no refs, not an alias key).
fn gc_virtual(tx: &Transaction, candidates: HashSet<String>) -> Result<()> {
    // Deepest first, so a child is gone before its parent is looked at. A parent that was kept because a child
    // still existed is looked at again when that child is deleted (it is queued then), so no key is skipped for
    // having been seen: every queued key is the parent of a row just deleted, which bounds the loop.
    let mut keys: Vec<String> = candidates.into_iter().collect();
    keys.sort_by_key(|k| std::cmp::Reverse(k.matches('/').count()));
    let mut queue: VecDeque<String> = keys.into();
    while let Some(key) = queue.pop_front() {
        let row: Option<(i64, Option<i64>)> = tx
            .query_row(
                "SELECT id, ns_parent_id FROM pages WHERE key = ?1 AND is_virtual = 1",
                [&key],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        let Some((id, parent)) = row else { continue };
        let has_children: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM pages WHERE ns_parent_id = ?1)",
            [id],
            |r| r.get(0),
        )?;
        let referenced: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM refs WHERE target_key = ?1) AND NOT EXISTS(SELECT 1 FROM aliases WHERE alias_key = ?1)",
            [&key],
            |r| r.get(0),
        )?;
        if has_children || referenced {
            continue;
        }
        tx.execute("DELETE FROM pages WHERE id = ?1", [id])?;
        if let Some(p) = parent {
            if let Some(pk) = tx
                .query_row("SELECT key FROM pages WHERE id = ?1", [p], |r| {
                    r.get::<_, String>(0)
                })
                .optional()?
            {
                queue.push_back(pk);
            }
        }
    }
    Ok(())
}

fn ref_kind_str(k: RefKind) -> &'static str {
    match k {
        RefKind::Page => "page",
        RefKind::Tag => "tag",
        RefKind::Block => "block",
        RefKind::BlockEmbed => "block_embed",
        RefKind::PageEmbed => "page_embed",
    }
}

fn insert_prop_rows(
    tx: &Transaction,
    kind: &str,
    owner: i64,
    key: &str,
    raw: &str,
    value: &PropertyValue,
) -> Result<()> {
    let mut st = tx.prepare_cached(
        "INSERT INTO props(owner_kind, owner_id, key, ord, value_text, value_fold, value_num, value_date, is_link) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
    )?;
    let scalar = |v: &PropertyValue, text: &str| -> (Option<f64>, Option<String>, String) {
        match v {
            PropertyValue::Number(n) => (Some(*n), None, text.to_string()),
            PropertyValue::Date(d) => (
                None,
                Some(d.format("%Y-%m-%d").to_string()),
                text.to_string(),
            ),
            PropertyValue::DateTime(d) => (
                None,
                Some(d.format("%Y-%m-%d").to_string()),
                text.to_string(),
            ),
            PropertyValue::Bool(b) => (None, None, b.to_string()),
            PropertyValue::Links(l) => (None, None, l.first().cloned().unwrap_or_default()),
            _ => (None, None, text.to_string()),
        }
    };
    match value {
        PropertyValue::List(items) => {
            let raws = split_top_level_commas(raw);
            for (i, item) in items.iter().enumerate() {
                // text items come from a YAML list (`[red, blue]`): use the parsed item, not a piece of the raw text
                let item_raw = match item {
                    PropertyValue::Text(s) => s.as_str(),
                    _ => raws.get(i).map_or("", |s| s.trim()),
                };
                let (n, d, t) = scalar(item, item_raw);
                st.execute(params![
                    kind,
                    owner,
                    key,
                    i as i64,
                    t,
                    search_fold(&t),
                    n,
                    d,
                    matches!(item, PropertyValue::Links(_)) as i64
                ])?;
            }
        }
        PropertyValue::Links(links) => {
            for (i, l) in links.iter().enumerate() {
                st.execute(params![
                    kind,
                    owner,
                    key,
                    i as i64,
                    l,
                    search_fold(l),
                    Option::<f64>::None,
                    Option::<String>::None,
                    1
                ])?;
            }
        }
        v => {
            let (n, d, t) = scalar(v, raw);
            st.execute(params![kind, owner, key, 0i64, t, search_fold(&t), n, d, 0])?;
        }
    }
    Ok(())
}

struct Ctx<'a> {
    tx: &'a Transaction<'a>,
    page_id: i64,
    p: &'a Parsed,
    seen_uids: HashSet<String>,
    duplicates: Vec<DuplicateId>,
    new_keys: HashSet<String>,
}

fn insert_block(
    ctx: &mut Ctx,
    b: &Block,
    parent_id: Option<i64>,
    ord: usize,
    path: &mut Vec<usize>,
) -> Result<()> {
    let tx = ctx.tx;
    let src = &ctx.p.text;
    let path_str = if b.kind == notatnik_core::BlockKind::Preamble {
        "p".to_string()
    } else {
        path.iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(".")
    };
    let mut uid: Option<String> = b.block_id.as_ref().map(|i| i.id.clone());
    if let Some(u) = &uid {
        let owned_elsewhere: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM blocks WHERE uid = ?1 AND page_id != ?2)",
            params![u, ctx.page_id],
            |r| r.get(0),
        )?;
        if owned_elsewhere || !ctx.seen_uids.insert(u.clone()) {
            ctx.duplicates.push(DuplicateId {
                path: path.clone(),
                id: u.clone(),
            });
            uid = None;
        }
    }
    let (task_state, sched, sched_t, dead, dead_t, prio) = match &b.task {
        Some(t) => (
            Some(t.state.name()),
            t.scheduled.map(|m| m.date.format("%Y-%m-%d").to_string()),
            t.scheduled
                .and_then(|m| m.time)
                .map(|x| x.format("%H:%M").to_string()),
            t.deadline.map(|m| m.date.format("%Y-%m-%d").to_string()),
            t.deadline
                .and_then(|m| m.time)
                .map(|x| x.format("%H:%M").to_string()),
            t.priority.clone(),
        ),
        None => (None, None, None, None, None, None),
    };
    tx.prepare_cached(
        "INSERT INTO blocks(page_id, parent_id, ord, path, level, uid, text, task_state, scheduled, scheduled_time, deadline, deadline_time, priority, priority_rank, span_start, span_end, is_preamble)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17)",
    )?
    .execute(params![
        ctx.page_id,
        parent_id,
        ord as i64,
        path_str,
        b.level as i64,
        uid,
        b.edit_text(src),
        task_state,
        sched,
        sched_t,
        dead,
        dead_t,
        prio.as_ref().map(|p| p.name().to_string()),
        prio.as_ref().map(|p| i64::from(p.rank())),
        b.span.start as i64,
        b.span.end as i64,
        i64::from(b.kind == notatnik_core::BlockKind::Preamble),
    ])?;
    let id = tx.last_insert_rowid();

    for p in &b.properties {
        insert_prop_rows(tx, "block", id, &p.key, &p.raw, &p.value)?;
    }
    for r in &b.refs {
        let kind = ref_kind_str(r.kind);
        let (target_key, target_uid) = match r.kind {
            RefKind::Block | RefKind::BlockEmbed => (None, Some(r.target.clone())),
            _ => {
                let k = key_of(&r.target);
                if let Ok(n) = PageName::parse(&r.target) {
                    ensure_page(tx, &n)?;
                }
                ctx.new_keys.insert(k.clone());
                (Some(k), None)
            }
        };
        tx.prepare_cached("INSERT INTO refs(src_page_id, src_block_id, kind, target_key, target_uid, raw) VALUES (?1,?2,?3,?4,?5,?6)")?
            .execute(params![ctx.page_id, id, kind, target_key, target_uid, r.span.slice(src)])?;
    }
    let content = search_fold(&b.search_text(src));
    if !content.trim().is_empty() {
        tx.prepare_cached("INSERT INTO fts(rowid, title, content) VALUES (?1, '', ?2)")?
            .execute(params![id, content])?;
    }
    path.push(0);
    for (i, c) in b.children.iter().enumerate() {
        if let Some(last) = path.last_mut() {
            *last = i;
        }
        insert_block(ctx, c, Some(id), i, path)?;
    }
    path.pop();
    Ok(())
}

fn write_page(tx: &Transaction, p: &Parsed) -> Result<ReindexOutcome> {
    let key = p.name.lookup_key();
    // Another file already owns this page name (e.g. `Alpha.md` and `alpha.md`)?
    let owner: Option<(i64, Option<String>, bool)> = tx
        .query_row(
            "SELECT id, file_path, is_virtual FROM pages WHERE key = ?1",
            [&key],
            |r| Ok((r.get(0)?, r.get(1)?, r.get::<_, i64>(2)? != 0)),
        )
        .optional()?;
    if let Some((_, Some(fp), false)) = &owner {
        if *fp != p.rel {
            tx.execute("DELETE FROM issues WHERE path = ?1", [&p.rel])?;
            tx.execute(
                "INSERT INTO issues(path, severity, code, message, offset) VALUES (?1,'error','duplicate-page',?2,NULL)",
                params![p.rel, format!("page `{}` is already defined by {fp}; this file is ignored", p.name)],
            )?;
            return Ok(ReindexOutcome::default());
        }
    }
    let page_id = ensure_page(tx, &p.name)?;
    let old = old_keys(tx, page_id)?;
    clear_page_content(tx, page_id)?;
    tx.execute("DELETE FROM issues WHERE path = ?1", [&p.rel])?;

    let fm = p.page.frontmatter.as_ref().filter(|f| f.valid);
    tx.execute(
        "UPDATE pages SET name = ?2, title = ?3, is_virtual = 0, file_path = ?4 WHERE id = ?1",
        params![
            page_id,
            p.name.as_str(),
            fm.and_then(|f| f.title.clone()),
            p.rel
        ],
    )?;

    let mut ctx = Ctx {
        tx,
        page_id,
        p,
        seen_uids: HashSet::new(),
        duplicates: Vec::new(),
        new_keys: HashSet::new(),
    };

    // page-level: aliases, tags, properties
    let mut alias_text = String::new();
    if let Some(f) = fm {
        for a in &f.aliases {
            if let Ok(n) = PageName::parse(a) {
                tx.execute(
                    "INSERT INTO aliases(alias_key, page_id) VALUES (?1, ?2)",
                    params![n.lookup_key(), page_id],
                )?;
                ctx.new_keys.insert(n.lookup_key());
                alias_text.push(' ');
                alias_text.push_str(n.as_str());
            }
        }
        for t in &f.tags {
            if let Ok(n) = PageName::parse(t) {
                ensure_page(tx, &n)?;
                let k = n.lookup_key();
                ctx.new_keys.insert(k.clone());
                tx.execute(
                    "INSERT INTO refs(src_page_id, src_block_id, kind, target_key, target_uid, raw) VALUES (?1, NULL, 'tag', ?2, NULL, ?3)",
                    params![page_id, k, t],
                )?;
            }
        }
        for e in &f.entries {
            insert_prop_rows(tx, "page", page_id, &e.key, &e.raw, &e.value)?;
            if let PropertyValue::Links(links) = &e.value {
                for l in links {
                    if let Ok(n) = PageName::parse(l) {
                        ensure_page(tx, &n)?;
                        let k = n.lookup_key();
                        ctx.new_keys.insert(k.clone());
                        tx.execute(
                            "INSERT INTO refs(src_page_id, src_block_id, kind, target_key, target_uid, raw) VALUES (?1, NULL, 'page', ?2, NULL, ?3)",
                            params![page_id, k, l],
                        )?;
                    }
                }
            }
        }
    }

    // a journal page has `type: journal` unless its frontmatter says otherwise: derived here, never written to
    // the file (ST-2), so icon rules, page queries and the type grouping see journals like any other page
    if p.name.journal_date().is_some()
        && !fm.is_some_and(|f| f.entries.iter().any(|e| e.key == "type"))
    {
        tx.execute(
            "INSERT INTO props(owner_kind, owner_id, key, ord, value_text, value_fold) VALUES ('page', ?1, 'type', 0, 'journal', 'journal')",
            [page_id],
        )?;
    }

    let mut path = Vec::new();
    if let Some(pre) = &p.page.preamble {
        insert_block(&mut ctx, pre, None, 0, &mut path)?;
    }
    for (i, b) in p.page.blocks.iter().enumerate() {
        path.clear();
        path.push(i);
        insert_block(&mut ctx, b, None, i, &mut path)?;
    }

    let title_text = format!(
        "{} {}{}",
        p.name,
        fm.and_then(|f| f.title.as_deref()).unwrap_or(""),
        alias_text
    );
    tx.execute(
        "INSERT INTO fts(rowid, title, content) VALUES (?1, ?2, '')",
        params![-page_id, search_fold(&title_text)],
    )?;

    // diagnostics → issues
    for d in &p.page.diagnostics {
        let sev = match d.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Info => "info",
        };
        tx.execute(
            "INSERT INTO issues(path, severity, code, message, offset) VALUES (?1,?2,?3,?4,?5)",
            params![p.rel, sev, d.code, d.message, d.span.start as i64],
        )?;
    }
    for d in &ctx.duplicates {
        tx.execute(
            "INSERT INTO issues(path, severity, code, message, offset) VALUES (?1,'warning','duplicate-id',?2,NULL)",
            params![p.rel, format!("block id `{}` was already used by another page and is being replaced", d.id)],
        )?;
    }

    tx.execute(
        "INSERT INTO files(path, page_id, mtime_ns, size, hash) VALUES (?1,?2,?3,?4,?5)
         ON CONFLICT(path) DO UPDATE SET page_id = ?2, mtime_ns = ?3, size = ?4, hash = ?5",
        params![p.rel, page_id, p.stat.mtime_ns, p.stat.size, p.hash],
    )?;

    let mut cands: HashSet<String> = old;
    cands.extend(ctx.new_keys.iter().cloned());
    cands.insert(key);
    for a in p.name.ancestors() {
        cands.insert(a.lookup_key());
    }
    let dups = std::mem::take(&mut ctx.duplicates);
    gc_virtual(tx, cands)?;
    Ok(ReindexOutcome {
        page: p.name.as_str().to_string(),
        removed: false,
        duplicates: dups,
    })
}

fn remove_path(tx: &Transaction, rel: &str) -> Result<()> {
    let row: Option<Option<i64>> = tx
        .query_row("SELECT page_id FROM files WHERE path = ?1", [rel], |r| {
            r.get(0)
        })
        .optional()?;
    tx.execute("DELETE FROM issues WHERE path = ?1", [rel])?;
    let Some(Some(page_id)) = row else {
        tx.execute("DELETE FROM files WHERE path = ?1", [rel])?;
        return Ok(());
    };
    let old = old_keys(tx, page_id)?;
    clear_page_content(tx, page_id)?;
    tx.execute(
        "UPDATE pages SET is_virtual = 1, file_path = NULL, title = NULL WHERE id = ?1",
        [page_id],
    )?;
    tx.execute("DELETE FROM files WHERE path = ?1", [rel])?;
    let (key, name): (String, String) = tx.query_row(
        "SELECT key, name FROM pages WHERE id = ?1",
        [page_id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let mut cands = old;
    cands.insert(key);
    if let Ok(n) = PageName::parse(&name) {
        for a in n.ancestors() {
            cands.insert(a.lookup_key());
        }
    }
    gc_virtual(tx, cands)
}

fn record_skip(tx: &Transaction, rel: &str, code: &str, message: &str) -> Result<()> {
    tx.execute("DELETE FROM issues WHERE path = ?1", [rel])?;
    tx.execute(
        "INSERT INTO issues(path, severity, code, message, offset) VALUES (?1,'error',?2,?3,NULL)",
        params![rel, code, message],
    )?;
    Ok(())
}

impl Index {
    /// Rebuilds the whole index from the files under `root` (cold index, NF-2).
    /// Files are processed in path order, so for duplicate block IDs the file that sorts first keeps the ID.
    pub fn index_all(&mut self, root: &Path) -> Result<Vec<(String, Vec<DuplicateId>)>> {
        let files = walk::scan(root)?;
        let opts = self.options.parse_options();
        let parsed: Vec<ReadResult> = files
            .pages
            .par_iter()
            .map(|f| read_parse(root, &f.rel, &opts))
            .collect();
        let tx = self.conn.transaction()?;
        for t in [
            "fts", "props", "refs", "aliases", "blocks", "files", "pages", "issues",
        ] {
            tx.execute(&format!("DELETE FROM {t}"), [])?;
        }
        let mut dups = Vec::new();
        for r in parsed {
            match r {
                ReadResult::Ok(p) => {
                    let out = write_page(&tx, &p)?;
                    if !out.duplicates.is_empty() {
                        dups.push((p.rel.clone(), out.duplicates));
                    }
                }
                ReadResult::Skip { rel, code, message } => record_skip(&tx, &rel, code, &message)?,
            }
        }
        tx.commit()?;
        self.fuzzy.invalidate();
        Ok(dups)
    }

    /// Reparses one file and updates the index (a deleted file is removed). `rel` is relative to `root`.
    pub fn reindex_file(&mut self, root: &Path, rel: &str) -> Result<ReindexOutcome> {
        let abs = root.join(rel);
        if !abs.exists() {
            return self.remove_file(rel);
        }
        if notatnik_core::path_to_page(rel).is_none() {
            return Ok(ReindexOutcome::default());
        }
        let opts = self.options.parse_options();
        let parsed = read_parse(root, rel, &opts);
        let tx = self.conn.transaction()?;
        let out = match parsed {
            ReadResult::Ok(p) => write_page(&tx, &p)?,
            ReadResult::Skip { rel, code, message } => {
                record_skip(&tx, &rel, code, &message)?;
                ReindexOutcome::default()
            }
        };
        tx.commit()?;
        self.fuzzy.invalidate();
        Ok(out)
    }

    /// Removes a file's page content from the index (the page stays as a virtual page if referenced).
    pub fn remove_file(&mut self, rel: &str) -> Result<ReindexOutcome> {
        let tx = self.conn.transaction()?;
        remove_path(&tx, rel)?;
        tx.commit()?;
        self.fuzzy.invalidate();
        Ok(ReindexOutcome {
            removed: true,
            ..ReindexOutcome::default()
        })
    }

    /// Startup reconciliation (P1.33): compares `(mtime, size)` with the `files` table, hashes only
    /// files that differ, and reindexes changed, new and deleted files.
    pub fn reconcile(&mut self, root: &Path) -> Result<ReconcileStats> {
        let files = walk::scan(root)?;
        let mut stored: BTreeMap<String, (i64, i64, String)> = BTreeMap::new();
        {
            let mut st = self
                .conn
                .prepare("SELECT path, mtime_ns, size, hash FROM files")?;
            for r in st.query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    (
                        r.get::<_, i64>(1)?,
                        r.get::<_, i64>(2)?,
                        r.get::<_, String>(3)?,
                    ),
                ))
            })? {
                let (p, v) = r?;
                stored.insert(p, v);
            }
        }
        let mut stats = ReconcileStats::default();
        let mut to_index: Vec<(String, bool)> = Vec::new(); // (rel, is_new)
        let mut touched: Vec<(String, FileStat)> = Vec::new();
        for f in &files.pages {
            let st = stat_of(&f.abs).map_err(Error::Io)?;
            match stored.remove(&f.rel) {
                None => to_index.push((f.rel.clone(), true)),
                Some((m, s, h)) => {
                    if m == st.mtime_ns && s == st.size {
                        continue;
                    }
                    stats.hashed += 1;
                    let bytes = std::fs::read(&f.abs)?;
                    if blake3::hash(&bytes).to_hex().as_str() == h {
                        touched.push((f.rel.clone(), st));
                    } else {
                        to_index.push((f.rel.clone(), false));
                    }
                }
            }
        }
        let opts = self.options.parse_options();
        let parsed: Vec<ReadResult> = to_index
            .par_iter()
            .map(|(rel, _)| read_parse(root, rel, &opts))
            .collect();
        let tx = self.conn.transaction()?;
        for (rel, st) in &touched {
            tx.execute(
                "UPDATE files SET mtime_ns = ?2, size = ?3 WHERE path = ?1",
                params![rel, st.mtime_ns, st.size],
            )?;
            stats.touched += 1;
        }
        for rel in stored.keys().cloned() {
            remove_path(&tx, &rel)?;
            stats.deleted += 1;
            stats.deleted_paths.push(rel);
        }
        for ((rel, is_new), r) in to_index.iter().zip(parsed) {
            match r {
                ReadResult::Ok(p) => {
                    let out = write_page(&tx, &p)?;
                    stats.reindexed += 1;
                    stats.reindexed_paths.push(rel.clone());
                    if *is_new {
                        stats.added += 1;
                    }
                    if !out.duplicates.is_empty() {
                        stats.duplicates.push((rel.clone(), out.duplicates));
                    }
                }
                ReadResult::Skip { rel, code, message } => record_skip(&tx, &rel, code, &message)?,
            }
        }
        tx.commit()?;
        self.fuzzy.invalidate();
        Ok(stats)
    }
}
