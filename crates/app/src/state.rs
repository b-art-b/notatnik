//! Application state and the command implementations.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use chrono::{Days, NaiveDate};
use notatnik_core::{
    BlockIdentity, BlockOp, BlockRef, PageName, ParseOptions, TextPatch, apply_patches,
    invert_patches, page_to_path, parse_page_with,
};
use notatnik_graph::{Dirs, Error as GraphError, Graph, Watcher};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::dto::*;
use crate::views::{PageInput, ViewOptions, find_block, page_view};

/// Error returned by every command.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct ApiError {
    /// Machine-readable code: `no_graph`, `not_found`, `conflict`, `invalid`, `undo_refused`, `internal`.
    pub code: String,
    /// Human-readable message.
    pub message: String,
}

impl ApiError {
    pub(crate) fn new(code: &str, message: impl Into<String>) -> Self {
        ApiError {
            code: code.into(),
            message: message.into(),
        }
    }
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for ApiError {}

impl From<GraphError> for ApiError {
    fn from(e: GraphError) -> Self {
        let code = match &e {
            GraphError::Conflict { .. } | GraphError::PartialRename { .. } => "conflict",
            GraphError::NotFound(_) => "not_found",
            GraphError::Edit(notatnik_core::EditError::Stale) => "conflict",
            GraphError::Edit(_)
            | GraphError::InvalidPath(_)
            | GraphError::Rename(_)
            | GraphError::Config(_) => "invalid",
            _ => "internal",
        };
        ApiError::new(code, e.to_string())
    }
}

impl From<notatnik_index::Error> for ApiError {
    fn from(e: notatnik_index::Error) -> Self {
        ApiError::new("internal", e.to_string())
    }
}

/// Result of a command.
pub type ApiResult<T> = Result<T, ApiError>;

/// Event sink: `(event name, JSON payload)`.
pub type Emitter = Arc<dyn Fn(&str, Value) + Send + Sync>;

enum UndoKind {
    Patches(Vec<TextPatch>),
    /// Undo of a page creation: delete the file.
    Delete,
    /// Redo of a page creation: write this text (the file must not exist).
    Create(String),
}

struct UndoEntry {
    page: String,
    rel: String,
    /// Hash the file must have (empty for `Create`: the file must not exist).
    expect_hash: String,
    kind: UndoKind,
}

#[derive(Default, Serialize, Deserialize)]
struct UiState {
    collapsed: BTreeMap<String, Vec<String>>,
}

struct Open {
    graph: Graph,
    _watcher: Option<Watcher>,
    trackers: HashMap<String, BlockIdentity>,
    undo: Vec<UndoEntry>,
    redo: Vec<UndoEntry>,
    ui: UiState,
    state_path: PathBuf,
    warnings: Vec<String>,
}

/// The application: one open graph at a time.
pub struct AppState {
    inner: Mutex<Option<Open>>,
    emit: Emitter,
    today: Arc<dyn Fn() -> NaiveDate + Send + Sync>,
    dirs: Dirs,
    watch: bool,
}

/// Options for [`AppState::new`].
pub struct AppOptions {
    /// Device directories.
    pub dirs: Dirs,
    /// Event sink.
    pub emit: Emitter,
    /// The clock (injectable for tests).
    pub today: Arc<dyn Fn() -> NaiveDate + Send + Sync>,
    /// Start a file watcher for the open graph.
    pub watch: bool,
}

impl AppOptions {
    /// Default options: system dirs and clock, watcher on, events dropped.
    pub fn standard(emit: Emitter) -> Self {
        AppOptions {
            dirs: Dirs::from_env(),
            emit,
            today: Arc::new(|| chrono::Local::now().date_naive()),
            watch: true,
        }
    }
}

fn now_seed() -> u64 {
    let n = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(1, |d| d.as_nanos() as u64);
    n | 1
}

/// Byte offset → UTF-16 code unit offset (what JavaScript string indices use).
fn utf16_offset(text: &str, byte: usize) -> u32 {
    text.get(..byte).map_or(0, |p| p.encode_utf16().count()) as u32
}

fn parent_path(p: &[u32]) -> Vec<usize> {
    p.iter().map(|x| *x as usize).collect()
}

impl AppState {
    /// Creates the state (no graph open yet).
    pub fn new(o: AppOptions) -> Self {
        AppState {
            inner: Mutex::new(None),
            emit: o.emit,
            today: o.today,
            dirs: o.dirs,
            watch: o.watch,
        }
    }

    fn lock(&self) -> MutexGuard<'_, Option<Open>> {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn with_open<R>(&self, f: impl FnOnce(&mut Open) -> ApiResult<R>) -> ApiResult<R> {
        let mut g = self.lock();
        let open = g
            .as_mut()
            .ok_or_else(|| ApiError::new("no_graph", "no graph is open"))?;
        f(open)
    }

    /// Today according to the injected clock.
    pub fn today(&self) -> NaiveDate {
        (self.today)()
    }

    // ---- commands -------------------------------------------------------------------------

    /// `open_graph`
    pub fn open_graph(&self, a: OpenGraphArgs) -> ApiResult<GraphInfo> {
        let path = PathBuf::from(&a.path);
        let (graph, report) =
            if a.create && !path.join(".notatnik").exists() && !path.join("pages").exists() {
                Graph::create(&path, &self.dirs)?
            } else {
                Graph::open(&path, &self.dirs)?
            };
        let emit = self.emit.clone();
        let watcher = if self.watch {
            Some(graph.watch_with(Duration::from_millis(200), move |ev| {
                emit(
                    "graph-changed",
                    json!({ "pages": ev.pages, "paths": ev.paths }),
                );
            })?)
        } else {
            None
        };
        let state_path = self.dirs.graph_data(&graph.id()).join("state.json");
        let ui: UiState = std::fs::read_to_string(&state_path)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default();
        let info = Self::info(&graph, report.warnings.clone());
        *self.lock() = Some(Open {
            graph,
            _watcher: watcher,
            trackers: HashMap::new(),
            undo: Vec::new(),
            redo: Vec::new(),
            ui,
            state_path,
            warnings: report.warnings,
        });
        Ok(info)
    }

    fn info(graph: &Graph, warnings: Vec<String>) -> GraphInfo {
        let root = graph.root().to_path_buf();
        GraphInfo {
            id: graph.id(),
            name: root
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            root: root.to_string_lossy().into_owned(),
            conflicts: graph.conflict_files().unwrap_or_default(),
            warnings,
            week_start: graph.config().graph.week_start,
        }
    }

    /// `graph_info`
    pub fn graph_info(&self, _a: NoArgs) -> ApiResult<GraphInfo> {
        self.with_open(|o| Ok(Self::info(&o.graph, o.warnings.clone())))
    }

    /// `today`
    pub fn today_cmd(&self, _a: NoArgs) -> ApiResult<StringValue> {
        Ok(StringValue {
            value: self.today().format("%Y-%m-%d").to_string(),
        })
    }

    fn build_view(o: &mut Open, name: &PageName) -> ApiResult<(PageView, String)> {
        let info = o.graph.with_index(|i| i.page_info(name.as_str()))?;
        let (canonical, rel, exists) = match &info {
            Some(i) => {
                let n = PageName::parse(&i.name).unwrap_or_else(|_| name.clone());
                let rel = i.file_path.clone().unwrap_or_else(|| page_to_path(&n));
                (n, rel, i.file_path.is_some() && !i.is_virtual)
            }
            None => (name.clone(), page_to_path(name), false),
        };
        let file = if exists {
            o.graph.read_page(&rel).ok()
        } else {
            None
        };
        let (text, hash, exists) = match file {
            Some(f) => (f.text, Some(f.hash), true),
            None => (String::new(), None, false),
        };
        let cfg = o.graph.config();
        let parsed = parse_page_with(
            &text,
            &ParseOptions {
                property_types: cfg.property_types(),
            },
        );
        let hidden: HashSet<String> = cfg.properties.hidden.iter().cloned().collect();
        let collapsed: HashSet<String> =
            o.ui.collapsed
                .get(canonical.as_str())
                .map(|v| v.iter().cloned().collect())
                .unwrap_or_default();
        let children = o
            .graph
            .with_index(|i| i.namespace_children(canonical.as_str()))?;
        let tracker = o.trackers.entry(rel.clone()).or_default();
        let view = page_view(
            PageInput {
                name: canonical.as_str(),
                path: &rel,
                exists,
                is_journal: canonical.journal_date().is_some(),
                hash,
                source: &text,
                page: &parsed,
                child_pages: children,
            },
            tracker,
            &ViewOptions {
                hidden: &hidden,
                collapsed: &collapsed,
            },
        );
        Ok((view, rel))
    }

    fn parse_name(s: &str) -> ApiResult<PageName> {
        PageName::parse(s)
            .map_err(|e| ApiError::new("invalid", format!("invalid page name `{s}`: {e}")))
    }

    /// `get_page`
    pub fn get_page(&self, a: GetPageArgs) -> ApiResult<PageView> {
        let name = Self::parse_name(&a.name)?;
        self.with_open(|o| Ok(Self::build_view(o, &name)?.0))
    }

    /// `apply_ops`
    pub fn apply_ops(&self, a: ApplyOpsArgs) -> ApiResult<ApplyResult> {
        let name = Self::parse_name(&a.page)?;
        self.with_open(|o| {
            let info = o.graph.with_index(|i| i.page_info(name.as_str()))?;
            let canonical = info
                .as_ref()
                .and_then(|i| PageName::parse(&i.name).ok())
                .unwrap_or_else(|| name.clone());
            let rel = info
                .and_then(|i| i.file_path)
                .unwrap_or_else(|| page_to_path(&canonical));
            let created = a.base_hash.is_none();
            let out = o.graph.edit_page(&rel, a.base_hash.as_deref(), &a.ops)?;
            if !out.patches.is_empty() || created {
                let entry = if created {
                    UndoEntry {
                        page: canonical.to_string(),
                        rel: rel.clone(),
                        expect_hash: out.hash.clone(),
                        kind: UndoKind::Delete,
                    }
                } else {
                    UndoEntry {
                        page: canonical.to_string(),
                        rel: rel.clone(),
                        expect_hash: out.hash.clone(),
                        kind: UndoKind::Patches(invert_patches(&out.patches)),
                    }
                };
                o.undo.push(entry);
                o.redo.clear();
                if o.undo.len() > 500 {
                    o.undo.remove(0);
                }
            }
            let (page, _) = Self::build_view(o, &canonical)?;
            Ok(ApplyResult { page })
        })
    }

    fn undo_redo(&self, undo: bool) -> ApiResult<UndoResult> {
        self.with_open(|o| {
            let entry = if undo { o.undo.pop() } else { o.redo.pop() }.ok_or_else(|| {
                ApiError::new(
                    "undo_refused",
                    if undo {
                        "nothing to undo"
                    } else {
                        "nothing to redo"
                    },
                )
            })?;
            let refuse = |m: &str| ApiError::new("undo_refused", m);
            let counter = match &entry.kind {
                UndoKind::Create(text) => match o.graph.read_page(&entry.rel) {
                    Err(GraphError::NotFound(_)) => {
                        let out = o.graph.write_page(&entry.rel, text, None)?;
                        UndoEntry {
                            page: entry.page.clone(),
                            rel: entry.rel.clone(),
                            expect_hash: out.hash,
                            kind: UndoKind::Delete,
                        }
                    }
                    _ => return Err(refuse("can't redo: the page exists again")),
                },
                UndoKind::Delete => {
                    let file = o
                        .graph
                        .read_page(&entry.rel)
                        .map_err(|_| refuse("can't undo: the page file is gone"))?;
                    if file.hash != entry.expect_hash {
                        return Err(refuse("can't undo: the page changed on disk"));
                    }
                    o.graph.delete_page(&entry.rel, &file.hash)?;
                    UndoEntry {
                        page: entry.page.clone(),
                        rel: entry.rel.clone(),
                        expect_hash: String::new(),
                        kind: UndoKind::Create(file.text),
                    }
                }
                UndoKind::Patches(p) => {
                    let file = o
                        .graph
                        .read_page(&entry.rel)
                        .map_err(|_| refuse("can't undo: the page file is gone"))?;
                    if file.hash != entry.expect_hash {
                        return Err(refuse("can't undo: the page changed on disk"));
                    }
                    let new_text = apply_patches(&file.text, p);
                    let out = o
                        .graph
                        .write_page(&entry.rel, &new_text, Some(&file.hash))?;
                    UndoEntry {
                        page: entry.page.clone(),
                        rel: entry.rel.clone(),
                        expect_hash: out.hash,
                        kind: UndoKind::Patches(invert_patches(p)),
                    }
                }
            };
            if undo {
                o.redo.push(counter);
            } else {
                o.undo.push(counter);
            }
            let name = Self::parse_name(&entry.page)?;
            let (page, _) = Self::build_view(o, &name)?;
            Ok(UndoResult { page })
        })
    }

    /// `undo`
    pub fn undo(&self, _a: NoArgs) -> ApiResult<UndoResult> {
        self.undo_redo(true)
    }

    /// `redo`
    pub fn redo(&self, _a: NoArgs) -> ApiResult<UndoResult> {
        self.undo_redo(false)
    }

    /// `search`
    pub fn search(&self, a: SearchArgs) -> ApiResult<Vec<SearchHitView>> {
        self.with_open(|o| {
            let hits = o
                .graph
                .with_index(|i| i.search(&a.text, a.limit as usize))?;
            Ok(hits
                .into_iter()
                .map(|h| SearchHitView {
                    kind: match h.kind {
                        notatnik_index::SearchKind::Page => "page".into(),
                        notatnik_index::SearchKind::Block => "block".into(),
                    },
                    page: h.page,
                    path: h
                        .path
                        .as_deref()
                        .filter(|p| *p != "p")
                        .map(|p| p.split('.').filter_map(|x| x.parse().ok()).collect()),
                    highlights: h
                        .highlights
                        .iter()
                        .map(|(s, e)| vec![utf16_offset(&h.text, *s), utf16_offset(&h.text, *e)])
                        .collect(),
                    text: h.text,
                })
                .collect())
        })
    }

    /// `search_titles`
    pub fn search_titles(&self, a: SearchArgs) -> ApiResult<Vec<TitleHit>> {
        self.with_open(|o| {
            let hits = o
                .graph
                .with_index_mut(|i| i.fuzzy_pages(&a.text, a.limit as usize))?;
            Ok(hits
                .into_iter()
                .map(|h| TitleHit {
                    name: h.name,
                    is_virtual: h.is_virtual,
                })
                .collect())
        })
    }

    /// `backlinks`
    pub fn backlinks(&self, a: PageArgs) -> ApiResult<Vec<BacklinkView>> {
        self.with_open(|o| {
            let links = o.graph.with_index(|i| i.backlinks(&a.page))?;
            let mut views: HashMap<String, PageView> = HashMap::new();
            let mut out = Vec::new();
            for l in links {
                let block_path: Option<Vec<u32>> = l
                    .block_path
                    .as_deref()
                    .map(|p| p.split('.').filter_map(|x| x.parse().ok()).collect());
                let block = match &block_path {
                    Some(bp) if !bp.is_empty() => {
                        if !views.contains_key(&l.source_page) {
                            let n = Self::parse_name(&l.source_page)?;
                            views.insert(l.source_page.clone(), Self::build_view(o, &n)?.0);
                        }
                        views
                            .get(&l.source_page)
                            .and_then(|v| find_block(&v.blocks, bp))
                            .map(|b| {
                                let mut b = b.clone();
                                b.children = Vec::new();
                                b
                            })
                    }
                    _ => None,
                };
                out.push(BacklinkView {
                    source_page: l.source_page,
                    block_path,
                    block,
                    breadcrumbs: l.breadcrumbs,
                    kind: l.kind,
                });
            }
            Ok(out)
        })
    }

    /// `list_namespace`
    pub fn list_namespace(&self, a: ListNamespaceArgs) -> ApiResult<Vec<NamespaceEntry>> {
        self.with_open(|o| {
            let rows = o
                .graph
                .with_index(|i| i.namespace_entries(a.parent.as_deref()))?;
            Ok(rows
                .into_iter()
                .map(|r| NamespaceEntry {
                    leaf: r.name.rsplit('/').next().unwrap_or(&r.name).to_string(),
                    name: r.name,
                    is_virtual: r.is_virtual,
                    has_children: r.descendants > 0,
                    descendants: r.descendants as u32,
                })
                .collect())
        })
    }

    /// `journals`
    pub fn journals(&self, a: JournalsArgs) -> ApiResult<Vec<JournalDay>> {
        let start = match &a.before {
            Some(d) => NaiveDate::parse_from_str(d, "%Y-%m-%d")
                .map_err(|_| ApiError::new("invalid", format!("invalid date `{d}`")))?
                .checked_sub_days(Days::new(1)),
            None => Some(self.today()),
        };
        let Some(start) = start else {
            return Ok(Vec::new());
        };
        self.with_open(|o| {
            let mut out = Vec::new();
            for i in 0..a.limit.min(366) {
                let Some(d) = start.checked_sub_days(Days::new(u64::from(i))) else {
                    break;
                };
                let name = d.format("%Y-%m-%d").to_string();
                let exists = o.graph.with_index(|ix| ix.page_has_file(&name))?;
                out.push(JournalDay {
                    date: name.clone(),
                    name,
                    exists,
                });
            }
            Ok(out)
        })
    }

    /// `rename_plan`
    pub fn rename_plan(&self, a: RenameArgs) -> ApiResult<RenamePlanView> {
        self.with_open(|o| {
            let p = o.graph.plan_rename(&a.old, &a.new)?;
            Ok(RenamePlanView {
                old: p.old,
                new: p.new,
                file_moves: p.file_moves.into_iter().map(|(a, b)| vec![a, b]).collect(),
                edits: p
                    .edits
                    .iter()
                    .map(|e| RenameEditView {
                        path: e.path.clone(),
                        replacements: e.patches.len() as u32,
                    })
                    .collect(),
                warnings: p.warnings,
            })
        })
    }

    /// `rename_apply`
    pub fn rename_apply(&self, a: RenameArgs) -> ApiResult<PageView> {
        self.with_open(|o| {
            let plan = o.graph.plan_rename(&a.old, &a.new)?;
            o.graph.apply_rename(&plan)?;
            // history of the old name can't be undone safely
            o.undo.retain(|e| e.page != plan.old);
            o.redo.retain(|e| e.page != plan.old);
            let n = Self::parse_name(&plan.new)?;
            Ok(Self::build_view(o, &n)?.0)
        })
    }

    /// `ensure_block_id`
    pub fn ensure_block_id(&self, a: EnsureBlockIdArgs) -> ApiResult<StringValue> {
        let name = Self::parse_name(&a.page)?;
        self.with_open(|o| {
            let (view, rel) = Self::build_view(o, &name)?;
            let block = find_block(&view.blocks, &a.path)
                .ok_or_else(|| ApiError::new("not_found", "no such block"))?;
            if let Some(uid) = &block.uid {
                return Ok(StringValue { value: uid.clone() });
            }
            let used = o.graph.with_index(|i| i.all_block_uids())?;
            let mut state = now_seed();
            let mut rng = move || {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                state
            };
            let id = notatnik_core::ids::generate_block_id(&used, &mut rng);
            let hash = view
                .hash
                .clone()
                .ok_or_else(|| ApiError::new("not_found", "page has no file"))?;
            o.graph.edit_page(
                &rel,
                Some(&hash),
                &[BlockOp::EnsureBlockId {
                    block: BlockRef {
                        path: parent_path(&a.path),
                        hash: Some(block.hash.clone()),
                    },
                    id: id.clone(),
                }],
            )?;
            Ok(StringValue { value: id })
        })
    }

    /// `get_block`
    pub fn get_block(&self, a: GetBlockArgs) -> ApiResult<Option<BlockLookup>> {
        self.with_open(|o| {
            let Some((page, path, _)) = o.graph.with_index(|i| i.block_by_uid(&a.uid))? else {
                return Ok(None);
            };
            let n = Self::parse_name(&page)?;
            let (view, _) = Self::build_view(o, &n)?;
            let p: Vec<u32> = path.split('.').filter_map(|x| x.parse().ok()).collect();
            Ok(find_block(&view.blocks, &p).map(|b| BlockLookup {
                page,
                block: b.clone(),
            }))
        })
    }

    /// `set_collapsed`
    pub fn set_collapsed(&self, a: SetCollapsedArgs) -> ApiResult<NoArgs> {
        self.with_open(|o| {
            if a.collapsed.is_empty() {
                o.ui.collapsed.remove(&a.page);
            } else {
                o.ui.collapsed.insert(a.page, a.collapsed);
            }
            if let Some(dir) = o.state_path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = std::fs::write(
                &o.state_path,
                serde_json::to_string_pretty(&o.ui).unwrap_or_default(),
            );
            Ok(NoArgs {})
        })
    }

    /// `list_conflicts`
    pub fn list_conflicts(&self, _a: NoArgs) -> ApiResult<StringList> {
        self.with_open(|o| {
            Ok(StringList {
                items: o.graph.conflict_files()?,
            })
        })
    }

    /// `read_conflict`
    pub fn read_conflict(&self, a: ReadConflictArgs) -> ApiResult<ConflictView> {
        self.with_open(|o| {
            if !o.graph.conflict_files()?.contains(&a.path) {
                return Err(ApiError::new("not_found", "not a conflict file"));
            }
            let text = std::fs::read_to_string(o.graph.root().join(&a.path))
                .map_err(|e| ApiError::new("internal", e.to_string()))?;
            let original_rel = a
                .path
                .find(" (conflicted copy ")
                .map(|i| format!("{}{}", &a.path[..i], ".md"));
            let original = original_rel
                .as_deref()
                .and_then(notatnik_core::path_to_page)
                .map(|n| n.to_string());
            let original_text = original_rel
                .as_deref()
                .and_then(|r| o.graph.read_page(r).ok())
                .map(|f| f.text);
            Ok(ConflictView {
                path: a.path,
                text,
                original,
                original_text,
            })
        })
    }

    /// The graph folder (for asset access by the shell).
    pub fn graph_root(&self) -> Option<PathBuf> {
        self.lock().as_ref().map(|o| o.graph.root().to_path_buf())
    }

    /// Reads a file below `assets/` (images).
    pub fn read_asset(&self, rel: &str) -> ApiResult<Vec<u8>> {
        self.with_open(|o| {
            let clean = rel.trim_start_matches("../").trim_start_matches("./");
            let p = Path::new(clean);
            if p.is_absolute()
                || p.components()
                    .any(|c| matches!(c, std::path::Component::ParentDir))
                || !clean.starts_with("assets/")
            {
                return Err(ApiError::new(
                    "invalid",
                    "assets must be inside the assets/ folder",
                ));
            }
            std::fs::read(o.graph.root().join(p))
                .map_err(|_| ApiError::new("not_found", "asset not found"))
        })
    }
}
