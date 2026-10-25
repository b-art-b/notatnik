//! Taking notes from one graph to another (MG-2 to MG-4): `transfer_plan` says what would happen, `transfer_apply`
//! does it, `transfer_undo` takes the last one back.
//!
//! Files on disk are plain in every graph and each graph encrypts what it uploads by its own rules, so a transfer
//! needs no key. It only warns where a page that is encrypted on one server would be plain on the other.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use notatnik_core::{
    Block, BlockOp, PageName, ParseOptions, RefKind, TextPatch, apply_patches, page_to_path,
    parse_page_with,
};
use notatnik_graph::{Graph, relative_asset_path};
use serde_json::json;

use super::{ApiError, ApiResult, AppState, Emitter};
use crate::dto::{
    ApplyOpsArgs, NoArgs, StringList, TransferArgs, TransferAsset, TransferItem, TransferPlan,
    TransferResult,
};
use crate::transfer::{
    AssetRef, defined_ids, find_asset_refs, ops_for_outline, outline_text, percent_decode,
    referenced_ids, rekey_ids, renamed_asset, rewrite_asset_refs, walk,
};

/// One of the two graphs of a transfer.
struct Side {
    graph: Graph,
    /// The slot it is open in, if it is.
    slot: Option<String>,
    emit: Option<Emitter>,
    /// Whether its key is in memory (`None`: not known, the graph is only opened for the transfer).
    unlocked: Option<bool>,
    /// The folder's name.
    name: String,
    id: String,
    has_server: bool,
    has_vault: bool,
}

impl Side {
    /// Is the file stored encrypted on this graph's server?
    fn encrypts(&self, rel: &str) -> bool {
        self.has_vault && self.graph.config().encryption.rules().is_encrypted(rel)
    }
}

/// What to do to the target graph.
enum Job {
    /// A page that does not exist: its whole text.
    Create {
        rel: String,
        name: String,
        text: String,
    },
    /// A page that exists, or one made from blocks: the blocks are added at the end.
    Edit {
        rel: String,
        name: String,
        base_hash: Option<String>,
        ops: Vec<BlockOp>,
    },
}

impl Job {
    fn name(&self) -> &str {
        match self {
            Job::Create { name, .. } | Job::Edit { name, .. } => name,
        }
    }
}

/// What to do to the source graph afterwards.
enum Removal {
    None,
    /// Move these pages to the trash.
    Pages(Vec<String>),
    /// Delete these blocks (paths, deepest and last first).
    Blocks {
        page: String,
        rel: String,
        hash: String,
        paths: Vec<Vec<usize>>,
    },
}

struct Prepared {
    plan: TransferPlan,
    jobs: Vec<Job>,
    /// `(file in the source, path in the target)`
    copies: Vec<(PathBuf, String)>,
    removal: Removal,
}

/// How to find a graph again for the undo.
#[derive(Clone)]
enum Where {
    Slot(String),
    Path(PathBuf),
}

/// What the last transfer did, to take it back (MG-4).
pub struct TransferUndo {
    target: Where,
    source: Where,
    /// Files made in the target.
    created: Vec<String>,
    /// `(file, text before, hash after)`: files of the target that blocks were added to.
    appended: Vec<(String, String, String)>,
    /// The trash entries of the source.
    trash: Vec<String>,
    /// `(file, text before, hash after)`: the page of the source that blocks were deleted from.
    deleted_from: Option<(String, String, String)>,
    pages: Vec<String>,
}

/// A hash of some bytes, short enough for a file name.
fn short_hash(bytes: &[u8]) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{h:016x}")[..8].to_string()
}

fn rng_from(seed: u64) -> impl FnMut() -> u64 {
    let mut s = seed | 1;
    move || {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        s
    }
}

fn is_page_ref(k: RefKind) -> bool {
    matches!(k, RefKind::Page | RefKind::Tag | RefKind::PageEmbed)
}

/// The new name of a page below (or at) `old` when `old` is now called `new`.
fn mapped(old: &PageName, new: &PageName, target: &str) -> Option<PageName> {
    let t = PageName::parse(target).ok()?;
    let (ok, tk) = (old.lookup_key(), t.lookup_key());
    if tk == ok {
        return Some(new.clone());
    }
    if tk.starts_with(&format!("{ok}/")) {
        let skip = old.as_str().split('/').count();
        let rest: Vec<&str> = t.as_str().split('/').skip(skip).collect();
        return PageName::parse(&format!("{new}/{}", rest.join("/"))).ok();
    }
    None
}

/// Points the page references of `text` that name `old` (or a page below it) at `new`.
fn rename_refs(text: &str, opts: &ParseOptions, old: &PageName, new: &PageName) -> String {
    let page = parse_page_with(text, opts);
    let mut patches: Vec<TextPatch> = Vec::new();
    let mut add = |b: &Block| {
        for r in &b.refs {
            if !is_page_ref(r.kind) {
                continue;
            }
            if let Some(n) = mapped(old, new, &r.target) {
                let (range, new_text) = if r.kind == RefKind::Tag
                    && !r.span.slice(text).starts_with("#[[")
                    && !n
                        .as_str()
                        .chars()
                        .all(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | '/'))
                {
                    (r.span, format!("#[[{n}]]"))
                } else {
                    (r.target_span, n.as_str().to_string())
                };
                patches.push(TextPatch {
                    range,
                    old_text: range.slice(text).to_string(),
                    new_text,
                });
            }
        }
    };
    if let Some(p) = &page.preamble {
        add(p);
    }
    walk(&page.blocks, &mut |_, b| add(b));
    apply_patches(text, &patches)
}

/// Page names (links, tags, embeds) that `text` points at.
fn linked_pages(text: &str, opts: &ParseOptions) -> Vec<String> {
    let page = parse_page_with(text, opts);
    let mut out = Vec::new();
    let mut add = |b: &Block| {
        for r in &b.refs {
            if matches!(r.kind, RefKind::Page | RefKind::PageEmbed) {
                out.push(r.target.clone());
            }
        }
    };
    if let Some(p) = &page.preamble {
        add(p);
    }
    walk(&page.blocks, &mut |_, b| add(b));
    out
}

fn dated_tasks(text: &str, opts: &ParseOptions) -> u32 {
    let page = parse_page_with(text, opts);
    let mut n = 0;
    walk(&page.blocks, &mut |_, b| {
        if b.task
            .as_ref()
            .is_some_and(|t| t.scheduled.is_some() || t.deadline.is_some())
        {
            n += 1;
        }
    });
    n
}

fn graph_name(g: &Graph) -> String {
    g.root()
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

impl AppState {
    fn on_slot<R>(&self, slot: &str, f: impl FnOnce(&AppState) -> ApiResult<R>) -> ApiResult<R> {
        if slot.is_empty() {
            return f(self);
        }
        let s = self
            .existing_slot(slot)
            .ok_or_else(|| ApiError::new("no_graph", "no graph is open in that slot"))?;
        f(&s)
    }

    fn side_of_slot(&self, slot: &str) -> ApiResult<Side> {
        self.on_slot(slot, |s| {
            let graph = s.with_open(|o| Ok(o.graph.clone()))?;
            Ok(self.side_of(
                graph,
                Some(slot.to_string()),
                Some(s.emit.clone()),
                Some(s.graph_keys_now().is_some()),
            ))
        })
    }

    fn side_of(
        &self,
        graph: Graph,
        slot: Option<String>,
        emit: Option<Emitter>,
        unlocked: Option<bool>,
    ) -> Side {
        let id = graph.id();
        let dev = notatnik_graph::DeviceSettings::load(&self.dirs).unwrap_or_default();
        Side {
            has_server: dev.remote_of(&id).is_some(),
            has_vault: graph.read_vault().ok().flatten().is_some(),
            name: graph_name(&graph),
            id,
            graph,
            slot,
            emit,
            unlocked,
        }
    }

    /// The target of a transfer: the graph of a slot, or a folder (opened for the transfer only, unless a slot has it).
    fn target_side(&self, a: &TransferArgs) -> ApiResult<Side> {
        if let Some(slot) = &a.to_slot {
            return self.side_of_slot(slot);
        }
        let path = a
            .to_path
            .as_deref()
            .ok_or_else(|| ApiError::new("invalid", "name the graph the notes go to"))?;
        let canon = std::fs::canonicalize(path)
            .map_err(|_| ApiError::new("not_found", format!("no folder `{path}`")))?;
        let open_in = self
            .open_roots
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .find(|(_, r)| **r == canon)
            .map(|(s, _)| s.clone());
        if let Some(slot) = open_in {
            return self.side_of_slot(&slot);
        }
        if !canon.join(".notatnik").exists() && !canon.join("pages").exists() {
            return Err(ApiError::new("invalid", "that folder is not a graph"));
        }
        let (graph, _) = Graph::open(&canon, &self.dirs)?;
        Ok(self.side_of(graph, None, None, None))
    }

    fn sides(&self, a: &TransferArgs) -> ApiResult<(Side, Side)> {
        let src = self.side_of_slot(&a.from)?;
        let dst = self.target_side(a)?;
        if src.id == dst.id {
            return Err(ApiError::new(
                "invalid",
                "choose another graph: the notes are in this one already",
            ));
        }
        Ok((src, dst))
    }

    /// Works out a transfer without writing anything.
    fn prepare(&self, a: &TransferArgs, src: &Side, dst: &Side) -> ApiResult<Prepared> {
        if !matches!(a.clash.as_str(), "keep_both" | "append" | "skip") {
            return Err(ApiError::new(
                "invalid",
                "clash is `keep_both`, `append` or `skip`",
            ));
        }
        let sopts = src.graph.config().parse_options();
        let dopts = dst.graph.config().parse_options();
        let page = PageName::parse(&a.page)
            .map_err(|e| ApiError::new("invalid", format!("bad page name: {e}")))?;
        let info = src
            .graph
            .with_index(|i| i.page_info(page.as_str()))?
            .ok_or_else(|| ApiError::new("not_found", format!("no page `{}`", a.page)))?;
        let exists_in_target = |name: &str| -> bool {
            dst.graph
                .with_index(|i| i.page_info(name))
                .ok()
                .flatten()
                .is_some_and(|i| i.file_path.is_some())
        };
        let mut taken: HashSet<String> = dst.graph.with_index(|i| i.all_block_uids())?;
        let mut rng = rng_from(super::now_seed());

        // what is taken: (name in the source, text after the work, how it goes into the target)
        let mut texts: Vec<String> = Vec::new();
        let mut items: Vec<TransferItem> = Vec::new();
        let mut jobs: Vec<Job> = Vec::new();
        let mut removal = Removal::None;
        let mut target_names: HashSet<String> = HashSet::new();
        // assets to look at once all texts are known: the texts are rewritten at the end
        struct Pending {
            job: usize,
            target_rel: String,
        }
        let mut pending: Vec<Pending> = Vec::new();

        match a.kind.as_str() {
            "pages" => {
                let mut pages: Vec<(String, String)> = Vec::new();
                if let Some(f) = info.file_path.clone() {
                    pages.push((info.name.clone(), f));
                }
                if a.include_subpages {
                    for s in src.graph.with_index(|i| i.subtree_pages(&info.name))? {
                        if let Some(f) = s.file_path {
                            pages.push((s.name, f));
                        }
                    }
                }
                if pages.is_empty() {
                    return Err(ApiError::new(
                        "invalid",
                        format!("`{}` has no file: there is nothing to take", info.name),
                    ));
                }
                let root = PageName::parse(&info.name)
                    .map_err(|e| ApiError::new("invalid", e.to_string()))?;
                // a name that exists in the target: keep both by calling the copy something else
                let clash_any = pages.iter().any(|(n, _)| {
                    PageName::parse(n).is_ok_and(|p| p.journal_date().is_none())
                        && exists_in_target(n)
                });
                let new_root = if clash_any && a.clash == "keep_both" {
                    let tag: String = src
                        .name
                        .chars()
                        .map(|c| if "[]|#/\\".contains(c) { '-' } else { c })
                        .collect();
                    let mut n = 1;
                    loop {
                        let cand = if n == 1 {
                            format!("{} (from {tag})", root.as_str())
                        } else {
                            format!("{} (from {tag} {n})", root.as_str())
                        };
                        let ok = PageName::parse(&cand)
                            .ok()
                            .filter(|p| !exists_in_target(p.as_str()));
                        if let Some(p) = ok {
                            break p;
                        }
                        n += 1;
                        if n > 99 {
                            return Err(ApiError::new("invalid", "no free name for the copy"));
                        }
                    }
                } else {
                    root.clone()
                };
                let renamed = new_root.as_str() != root.as_str();
                for (name, rel) in &pages {
                    let src_page = PageName::parse(name)
                        .map_err(|e| ApiError::new("invalid", e.to_string()))?;
                    let tgt = if renamed {
                        mapped(&root, &new_root, name).unwrap_or_else(|| src_page.clone())
                    } else {
                        src_page.clone()
                    };
                    let exists = exists_in_target(tgt.as_str());
                    let file = src.graph.read_page(rel)?;
                    let parsed = parse_page_with(&file.text, &sopts);
                    let encrypted_in_source = src.encrypts(rel);
                    let action = match (exists, a.clash.as_str()) {
                        (false, _) => "create",
                        (true, "skip") => "skip",
                        (true, _) => "append",
                    };
                    let tgt_rel = if action == "append" {
                        dst.graph
                            .with_index(|i| i.page_info(tgt.as_str()))?
                            .and_then(|i| i.file_path)
                            .unwrap_or_else(|| page_to_path(&tgt))
                    } else {
                        page_to_path(&tgt)
                    };
                    items.push(TransferItem {
                        source: name.clone(),
                        target: tgt.to_string(),
                        action: action.into(),
                        renamed: tgt.as_str() != src_page.as_str(),
                        blocks: parsed.blocks.len() as u32,
                        encrypted_in_source,
                        encrypted_in_target: dst.encrypts(&tgt_rel),
                    });
                    if action == "skip" {
                        continue;
                    }
                    target_names.insert(tgt.lookup_key());
                    if action == "create" {
                        let mut text = file.text.clone();
                        if renamed {
                            text = rename_refs(&text, &sopts, &root, &new_root);
                        }
                        let (text, _) = rekey_ids(&text, &dopts, &mut taken, &mut rng);
                        pending.push(Pending {
                            job: jobs.len(),
                            target_rel: tgt_rel.clone(),
                        });
                        jobs.push(Job::Create {
                            rel: tgt_rel,
                            name: tgt.to_string(),
                            text,
                        });
                    } else {
                        let tops: Vec<&Block> = parsed.blocks.iter().collect();
                        let mut outline = outline_text(&file.text, &tops);
                        if renamed {
                            outline = rename_refs(&outline, &sopts, &root, &new_root);
                        }
                        let (outline, _) = rekey_ids(&outline, &dopts, &mut taken, &mut rng);
                        let current = dst.graph.read_page(&tgt_rel)?;
                        let existing = parse_page_with(&current.text, &dopts).blocks.len();
                        pending.push(Pending {
                            job: jobs.len(),
                            target_rel: tgt_rel.clone(),
                        });
                        jobs.push(Job::Edit {
                            rel: tgt_rel,
                            name: tgt.to_string(),
                            base_hash: Some(current.hash),
                            // the outline is kept in `texts` for the checks; the ops are made after the assets
                            ops: vec![BlockOp::Insert {
                                parent: vec![],
                                index: existing,
                                text: outline,
                            }],
                        });
                    }
                }
                if a.remove {
                    let moved: Vec<String> = items
                        .iter()
                        .filter(|i| i.action != "skip")
                        .map(|i| i.source.clone())
                        .collect();
                    if !moved.is_empty() {
                        removal = Removal::Pages(moved);
                    }
                }
            }
            "blocks" => {
                let rel = info.file_path.clone().ok_or_else(|| {
                    ApiError::new("invalid", format!("`{}` has no file", info.name))
                })?;
                if a.blocks.is_empty() {
                    return Err(ApiError::new("invalid", "no blocks are selected"));
                }
                let file = src.graph.read_page(&rel)?;
                let parsed = parse_page_with(&file.text, &sopts);
                // the top blocks of the selection: a block whose parent is selected too comes with the parent
                let mut paths: Vec<Vec<usize>> = a
                    .blocks
                    .iter()
                    .map(|p| p.iter().map(|x| *x as usize).collect())
                    .collect();
                paths.sort();
                paths.dedup();
                let all = paths.clone();
                paths.retain(|p| !all.iter().any(|q| q.len() < p.len() && p.starts_with(q)));
                let mut tops: Vec<&Block> = Vec::new();
                for p in &paths {
                    let mut level: &[Block] = &parsed.blocks;
                    let mut found: Option<&Block> = None;
                    for i in p {
                        found = level.get(*i);
                        level = found.map_or(&[], |b| &b.children);
                    }
                    tops.push(found.ok_or_else(|| {
                        ApiError::new("not_found", "a selected block is not there any more")
                    })?);
                }
                let tgt = PageName::parse(a.target_page.as_deref().unwrap_or(&a.page))
                    .map_err(|e| ApiError::new("invalid", format!("bad page name: {e}")))?;
                let existing_info = dst.graph.with_index(|i| i.page_info(tgt.as_str()))?;
                let (tgt_name, tgt_rel, current) =
                    match existing_info.filter(|i| i.file_path.is_some()) {
                        Some(i) => {
                            let rel = i.file_path.unwrap_or_default();
                            let cur = dst.graph.read_page(&rel)?;
                            (i.name, rel, Some(cur))
                        }
                        None => (tgt.to_string(), page_to_path(&tgt), None),
                    };
                let outline = outline_text(&file.text, &tops);
                let (outline, _) = rekey_ids(&outline, &dopts, &mut taken, &mut rng);
                let existing = current
                    .as_ref()
                    .map_or(0, |c| parse_page_with(&c.text, &dopts).blocks.len());
                items.push(TransferItem {
                    source: info.name.clone(),
                    target: tgt_name.clone(),
                    action: if current.is_some() {
                        "append"
                    } else {
                        "create"
                    }
                    .into(),
                    renamed: tgt_name != info.name,
                    blocks: tops.len() as u32,
                    encrypted_in_source: src.encrypts(&rel),
                    encrypted_in_target: dst.encrypts(&tgt_rel),
                });
                target_names.insert(tgt.lookup_key());
                pending.push(Pending {
                    job: 0,
                    target_rel: tgt_rel.clone(),
                });
                jobs.push(Job::Edit {
                    rel: tgt_rel,
                    name: tgt_name,
                    base_hash: current.map(|c| c.hash),
                    ops: vec![BlockOp::Insert {
                        parent: vec![],
                        index: existing,
                        text: outline,
                    }],
                });
                if a.remove {
                    let mut del = paths.clone();
                    del.sort();
                    del.reverse();
                    removal = Removal::Blocks {
                        page: info.name.clone(),
                        rel,
                        hash: file.hash,
                        paths: del,
                    };
                }
            }
            other => {
                return Err(ApiError::new(
                    "invalid",
                    format!("kind is `pages` or `blocks`, not `{other}`"),
                ));
            }
        }

        // ---- pictures and attachments --------------------------------------------------------------------
        // the text of each job, to look for files in
        let job_text = |j: &Job| -> String {
            match j {
                Job::Create { text, .. } => text.clone(),
                Job::Edit { ops, .. } => match ops.first() {
                    Some(BlockOp::Insert { text, .. }) => text.clone(),
                    _ => String::new(),
                },
            }
        };
        let mut assets: Vec<TransferAsset> = Vec::new();
        let mut copies: Vec<(PathBuf, String)> = Vec::new();
        // decoded source path -> (new path as written, action)
        let mut decided: HashMap<String, (String, String)> = HashMap::new();
        let mut asset_plain_in_target = 0usize;
        for p in &pending {
            let text = job_text(&jobs[p.job]);
            for r in find_asset_refs(&text) {
                let decoded = percent_decode(&r.path);
                if decided.contains_key(&decoded) || decoded.contains("..") {
                    continue;
                }
                let from = src.graph.root().join(&decoded);
                let Ok(bytes) = std::fs::read(&from)
                    .map_err(|_| ())
                    .and_then(|b| from.is_file().then_some(b).ok_or(()))
                else {
                    assets.push(TransferAsset {
                        path: decoded.clone(),
                        bytes: 0,
                        action: "missing".into(),
                        new_path: decoded.clone(),
                    });
                    decided.insert(decoded, (r.path.clone(), "missing".into()));
                    continue;
                };
                let to = dst.graph.root().join(&decoded);
                let (action, written, new_decoded) = match std::fs::read(&to) {
                    Ok(existing) if existing == bytes => ("reuse", r.path.clone(), decoded.clone()),
                    Ok(_) => {
                        let h = short_hash(&bytes);
                        (
                            "rename",
                            renamed_asset(&r.path, &h),
                            renamed_asset(&decoded, &h),
                        )
                    }
                    Err(_) => ("copy", r.path.clone(), decoded.clone()),
                };
                if src.encrypts(&decoded) && !dst.encrypts(&new_decoded) {
                    asset_plain_in_target += 1;
                }
                if action != "reuse" {
                    copies.push((from, new_decoded.clone()));
                }
                assets.push(TransferAsset {
                    path: decoded.clone(),
                    bytes: bytes.len() as u64,
                    action: action.into(),
                    new_path: new_decoded,
                });
                decided.insert(decoded, (written, action.into()));
            }
        }
        // point the texts at the files in the target
        for p in &pending {
            let text = job_text(&jobs[p.job]);
            let refs: Vec<AssetRef> = find_asset_refs(&text);
            let new_text = rewrite_asset_refs(&text, &refs, |r| {
                let (written, action) = decided.get(&percent_decode(&r.path))?;
                (action != "missing").then(|| relative_asset_path(&p.target_rel, written))
            });
            match &mut jobs[p.job] {
                Job::Create { text, .. } => *text = new_text,
                Job::Edit { ops, .. } => {
                    // now the outline is final: the blocks as inserts
                    let existing = match ops.first() {
                        Some(BlockOp::Insert { index, .. }) => *index,
                        _ => 0,
                    };
                    *ops = ops_for_outline(&new_text, existing, &dopts);
                }
            }
        }
        for j in &jobs {
            texts.push(match j {
                Job::Create { text, .. } => text.clone(),
                Job::Edit { ops, .. } => ops
                    .iter()
                    .filter_map(|o| match o {
                        BlockOp::Insert { text, .. } => Some(format!("- {text}\n")),
                        _ => None,
                    })
                    .collect(),
            });
        }

        // ---- what the person should know ---------------------------------------------------------------
        let mut missing_links: Vec<String> = Vec::new();
        let mut seen_links: HashSet<String> = HashSet::new();
        let (mut defined, mut referenced): (HashSet<String>, HashSet<String>) =
            (HashSet::new(), HashSet::new());
        let mut dated = 0;
        let mut queries = false;
        for t in &texts {
            for l in linked_pages(t, &dopts) {
                let Ok(n) = PageName::parse(&l) else { continue };
                let key = n.lookup_key();
                if n.journal_date().is_some()
                    || target_names.contains(&key)
                    || !seen_links.insert(key)
                {
                    continue;
                }
                let has = dst
                    .graph
                    .with_index(|i| i.page_info(n.as_str()))
                    .ok()
                    .flatten()
                    .is_some();
                if !has {
                    missing_links.push(n.to_string());
                }
            }
            defined.extend(defined_ids(t, &dopts));
            referenced.extend(referenced_ids(t, &dopts));
            dated += dated_tasks(t, &dopts);
            queries |= t.contains("```query") || t.contains("{{query");
        }
        missing_links.truncate(100);
        let dangling = referenced.difference(&defined).count() as u32;

        let mut left_behind: Vec<String> = Vec::new();
        if a.remove {
            let moved: Vec<String> = items
                .iter()
                .filter(|i| i.action != "skip")
                .map(|i| i.source.clone())
                .collect();
            if a.kind == "pages" {
                for n in &moved {
                    for l in src.graph.with_index(|i| i.backlinks(n))? {
                        if !moved.contains(&l.source_page) && !left_behind.contains(&l.source_page)
                        {
                            left_behind.push(l.source_page);
                        }
                    }
                }
            }
        }
        left_behind.truncate(100);

        let mut warnings: Vec<String> = Vec::new();
        let leaks = items
            .iter()
            .filter(|i| i.action != "skip" && i.encrypted_in_source && !i.encrypted_in_target)
            .count()
            + asset_plain_in_target;
        if leaks > 0 && dst.has_server {
            warnings.push(format!(
                "{leaks} file(s) are stored encrypted on the server of {} but would be plain text on the server of {}.",
                src.name, dst.name
            ));
        }
        let target_locked = dst.has_vault
            && dst.unlocked == Some(false)
            && items
                .iter()
                .any(|i| i.action != "skip" && i.encrypted_in_target);
        if target_locked {
            warnings.push(format!(
                "{} has encryption and is locked: the notes are written, and they are uploaded encrypted once it is unlocked.",
                dst.name
            ));
        }
        if queries {
            warnings.push(format!(
                "The notes hold queries: they run on the pages of {} from now on.",
                dst.name
            ));
        }
        if dated > 0 {
            warnings.push(format!(
                "{dated} task(s) with a date: if both graphs use the same calendar, they appear there once for each graph."
            ));
        }

        Ok(Prepared {
            plan: TransferPlan {
                from_name: src.name.clone(),
                to_name: dst.name.clone(),
                from_id: src.id.clone(),
                to_id: dst.id.clone(),
                removes: a.remove,
                items,
                assets,
                missing_links,
                left_behind,
                dangling_refs: dangling,
                dated_tasks: dated,
                target_locked,
                warnings,
            },
            jobs,
            copies,
            removal,
        })
    }

    /// `transfer_plan`: what taking these notes to another graph would do. Nothing is written.
    pub fn transfer_plan(&self, a: TransferArgs) -> ApiResult<TransferPlan> {
        let (src, dst) = self.sides(&a)?;
        Ok(self.prepare(&a, &src, &dst)?.plan)
    }

    fn emit_changed(side: &Side, pages: &[String], rels: &[String]) {
        if let Some(emit) = &side.emit {
            emit(
                "graph-changed",
                json!({ "pages": pages, "paths": rels, "own": false }),
            );
        }
    }

    /// `transfer_apply`: takes the notes to the other graph. With `remove`, they leave the source graph (its trash, or
    /// the blocks are deleted).
    pub fn transfer_apply(&self, a: TransferArgs) -> ApiResult<TransferResult> {
        let _one_at_a_time = self
            .transfer_lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let (src, dst) = self.sides(&a)?;
        let prepared = self.prepare(&a, &src, &dst)?;
        let Prepared {
            plan,
            jobs,
            copies,
            removal,
        } = prepared;

        let mut undo = TransferUndo {
            target: dst
                .slot
                .clone()
                .map_or_else(|| Where::Path(dst.graph.root().to_path_buf()), Where::Slot),
            source: Where::Slot(a.from.clone()),
            created: Vec::new(),
            appended: Vec::new(),
            trash: Vec::new(),
            deleted_from: None,
            pages: Vec::new(),
        };

        // 1. the files the text points at (a new file only; nothing that is there is changed)
        for (from, to_rel) in &copies {
            let io = |e: std::io::Error| ApiError::new("internal", format!("{to_rel}: {e}"));
            let bytes = std::fs::read(from).map_err(io)?;
            let to = dst.graph.root().join(to_rel);
            if let Some(dir) = to.parent() {
                std::fs::create_dir_all(dir).map_err(io)?;
            }
            std::fs::write(&to, bytes).map_err(io)?;
        }

        // 2. the pages
        let mut written: Vec<String> = Vec::new();
        let mut rels: Vec<String> = Vec::new();
        let mut failure: Option<ApiError> = None;
        for j in &jobs {
            let r = match j {
                Job::Create { rel, text, .. } => dst
                    .graph
                    .write_page(rel, text, None)
                    .map(|_| (rel.clone(), None)),
                Job::Edit {
                    rel,
                    base_hash,
                    ops,
                    ..
                } => {
                    let before = dst.graph.read_page(rel).ok().map(|f| f.text);
                    dst.graph
                        .edit_page(rel, base_hash.as_deref(), ops)
                        .map(|o| (rel.clone(), before.map(|b| (b, o.hash))))
                }
            };
            match r {
                Ok((rel, None)) => {
                    undo.created.push(rel.clone());
                    rels.push(rel);
                    written.push(j.name().to_string());
                }
                Ok((rel, Some((before, hash)))) => {
                    undo.appended.push((rel.clone(), before, hash));
                    rels.push(rel);
                    written.push(j.name().to_string());
                }
                Err(e) => {
                    failure = Some(e.into());
                    break;
                }
            }
        }
        undo.pages = written.clone();
        if let Some(e) = failure {
            // nothing is removed from the source; put the target back as it was
            log::warn!("a transfer stopped ({e}); the pages it had written are taken back");
            let _ = self.undo_target(&dst, &undo);
            return Err(e);
        }
        Self::emit_changed(&dst, &written, &rels);

        // 3. the source
        let mut undoable = false;
        match removal {
            Removal::None => {}
            Removal::Pages(names) => {
                let done = self.on_slot(&a.from, |s| {
                    s.with_open(|o| {
                        let mut pages = Vec::new();
                        for n in &names {
                            if let Some(f) = o
                                .graph
                                .with_index(|i| i.page_info(n))?
                                .and_then(|i| i.file_path)
                            {
                                pages.push((n.clone(), f));
                            }
                        }
                        if pages.is_empty() {
                            return Ok(None);
                        }
                        s.delete_found(o, pages).map(Some)
                    })
                })?;
                if let Some(d) = done {
                    undo.trash.push(d.trash_id);
                    undoable = true;
                    // the pane of the source may be another window's: it hears that the pages are gone
                    Self::emit_changed(&src, &d.deleted, &[]);
                }
            }
            Removal::Blocks {
                page,
                rel,
                hash,
                paths,
            } => {
                let before = src.graph.read_page(&rel)?.text;
                let ops: Vec<BlockOp> = paths
                    .iter()
                    .map(|p| BlockOp::Delete {
                        block: notatnik_core::BlockRef::at(p),
                    })
                    .collect();
                self.on_slot(&a.from, |s| {
                    s.apply_ops(ApplyOpsArgs {
                        page: page.clone(),
                        base_hash: Some(hash),
                        ops,
                    })
                })?;
                let after = src.graph.read_page(&rel)?.hash;
                Self::emit_changed(
                    &src,
                    std::slice::from_ref(&page),
                    std::slice::from_ref(&rel),
                );
                undo.deleted_from = Some((rel, before, after));
                undoable = true;
            }
        }
        // copying can be taken back as well: the pages it made go to the target's trash
        undoable |= !undo.created.is_empty() || !undo.appended.is_empty();
        log::info!(
            "transfer: {} page(s) from {} to {}{}",
            written.len(),
            src.name,
            dst.name,
            if a.remove { " (moved)" } else { "" }
        );
        *self
            .last_transfer
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(undo);
        Ok(TransferResult {
            plan,
            written,
            undoable,
        })
    }

    /// Takes back what a transfer wrote in the target: the files it made go to the trash, the text it added to a page
    /// is taken out again (only while the page is as the transfer left it). Returns what could not be taken back.
    fn undo_target(&self, dst: &Side, u: &TransferUndo) -> Vec<String> {
        let mut left = Vec::new();
        let exist: Vec<String> = u
            .created
            .iter()
            .filter(|r| dst.graph.root().join(r).is_file())
            .cloned()
            .collect();
        if !exist.is_empty() {
            if let Err(e) = dst.graph.trash_pages(&exist) {
                left.push(format!("{}: {e}", exist.join(", ")));
            }
        }
        for (rel, before, after) in &u.appended {
            match dst.graph.read_page(rel) {
                Ok(f) if &f.hash == after => {
                    if let Err(e) = dst.graph.write_page(rel, before, Some(&f.hash)) {
                        left.push(format!("{rel}: {e}"));
                    }
                }
                _ => left.push(format!("{rel} was changed after the transfer")),
            }
        }
        Self::emit_changed(dst, &u.pages, &u.created);
        left
    }

    /// `transfer_undo`: takes the last transfer back: the target loses what it got, the source gets back what it lost.
    /// Returns what could not be taken back because it was changed since.
    pub fn transfer_undo(&self, _a: NoArgs) -> ApiResult<StringList> {
        let _one_at_a_time = self
            .transfer_lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(u) = self
            .last_transfer
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
        else {
            return Err(ApiError::new("invalid", "there is no transfer to undo"));
        };
        let side = |w: &Where| -> ApiResult<Side> {
            match w {
                Where::Slot(s) => self.side_of_slot(s),
                Where::Path(p) => {
                    let (graph, _) = Graph::open(p, &self.dirs)?;
                    Ok(self.side_of(graph, None, None, None))
                }
            }
        };
        let dst = side(&u.target)?;
        let src = side(&u.source)?;
        let mut left = self.undo_target(&dst, &u);
        let mut back: Vec<String> = Vec::new();
        for id in &u.trash {
            match src.graph.restore_trash(id) {
                Ok(r) => back.extend(r),
                Err(e) => left.push(format!("the trash entry {id}: {e}")),
            }
        }
        if let Some((rel, before, after)) = &u.deleted_from {
            match src.graph.read_page(rel) {
                Ok(f) if &f.hash == after => {
                    match src.graph.write_page(rel, before, Some(&f.hash)) {
                        Ok(_) => back.push(rel.clone()),
                        Err(e) => left.push(format!("{rel}: {e}")),
                    }
                }
                _ => left.push(format!("{rel} was changed after the transfer")),
            }
        }
        if !back.is_empty() {
            let names: Vec<String> = back
                .iter()
                .filter_map(|r| src.graph.with_index(|i| i.page_for_file(r)).ok().flatten())
                .collect();
            Self::emit_changed(&src, &names, &back);
        }
        Ok(StringList { items: left })
    }
}
