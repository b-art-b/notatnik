//! Application state and the command implementations.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use chrono::{Days, NaiveDate};
use notatnik_core::{
    BlockIdentity, BlockOp, BlockRef, PageName, TextPatch, apply_patches, invert_patches,
    page_to_path, parse_page_with,
};
use notatnik_graph::{Dirs, Error as GraphError, Graph, Watcher};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::dto::*;
use crate::templates;

#[derive(Clone, Copy, PartialEq, Eq)]
enum TemplateMode {
    Type,
    NewPage,
}

struct TemplateOps {
    file: String,
    ops: Vec<BlockOp>,
    added: Vec<String>,
    top_level: usize,
}
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
            GraphError::Locked(_) => "locked",
            GraphError::Crypt(_) => "invalid",
            GraphError::Edit(notatnik_core::EditError::Stale) => "conflict",
            GraphError::Edit(_)
            | GraphError::InvalidPath(_)
            | GraphError::Rename(_)
            | GraphError::Asset(_)
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

/// The thread that takes the automatic snapshots (BK-3). Dropping it (graph closed or replaced) stops the thread.
struct BackupTask {
    stop: Arc<AtomicBool>,
    error: Arc<Mutex<Option<String>>>,
    /// Copying to the second folder failed (BK-9).
    mirror_error: Arc<Mutex<Option<String>>>,
}

fn diff_line(op: notatnik_graph::DiffOp) -> DiffLine {
    use notatnik_graph::DiffOp;
    let (kind, text) = match op {
        DiffOp::Same(t) => ("same", t),
        DiffOp::Add(t) => ("add", t),
        DiffOp::Del(t) => ("del", t),
        DiffOp::Gap => ("gap", String::new()),
    };
    DiffLine {
        kind: kind.into(),
        text,
    }
}

fn take<T: Clone>(m: &Mutex<T>) -> T {
    m.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone()
}

fn put<T>(m: &Mutex<T>, v: T) {
    *m.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = v;
}

/// The first snapshot folder of the device settings: the setting, or the default.
fn primary_dir(dirs: &Dirs, dev: &notatnik_graph::DeviceSettings) -> PathBuf {
    match dev.ui.backup_dir.trim() {
        "" => dirs.data.join("backups"),
        d => PathBuf::from(d),
    }
}

/// The second snapshot folder, if one is set.
fn mirror_dir(dev: &notatnik_graph::DeviceSettings) -> Option<PathBuf> {
    Some(dev.ui.backup_mirror_dir.trim())
        .filter(|d| !d.is_empty())
        .map(PathBuf::from)
}

/// Backup keys stay in memory this long after their last use.
const BACKUP_LOCK_AFTER: Duration = Duration::from_secs(15 * 60);

/// The unlocked backup vault (EN-2): in memory only, wiped when dropped.
struct Unlocked {
    keys: Arc<notatnik_crypt::Keys>,
    last_use: std::time::Instant,
}

fn vault_path(dirs: &Dirs) -> PathBuf {
    dirs.config.join("backup-vault.json")
}

/// A vault made but not yet confirmed (the recovery key was shown, the user has not said it is saved).
fn pending_vault_path(dirs: &Dirs) -> PathBuf {
    dirs.config.join("backup-vault.pending.json")
}

fn load_vault(path: &Path) -> Result<Option<notatnik_crypt::Vault>, String> {
    match std::fs::read(path) {
        Ok(b) => notatnik_crypt::Vault::from_json(&b)
            .map(Some)
            .map_err(|e| format!("{}: {e}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("{}: {e}", path.display())),
    }
}

/// Writes a descriptor next to its final place, then renames it: a crash leaves the old one.
fn save_vault(path: &Path, vault: &notatnik_crypt::Vault) -> std::io::Result<()> {
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d)?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, vault.to_json())?;
    std::fs::rename(&tmp, path)
}

/// How snapshots are written now (BK-7, BK-8, EN-5): the compression setting, the device files that go with them
/// and, when encryption is on, the vault to seal to. With `for_writing` an enabled encryption whose vault cannot
/// be read is an error: a backup must never be written in plain by accident. `keys` are the unlocked keys, if any.
fn backup_options(
    dirs: &Dirs,
    dev: &notatnik_graph::DeviceSettings,
    graph_id: &str,
    keys: Option<Arc<notatnik_crypt::Keys>>,
    for_writing: bool,
) -> Result<notatnik_graph::BackupOptions, String> {
    use notatnik_graph::Extra;
    let seal_to = if dev.ui.backup_encrypt && for_writing {
        match load_vault(&vault_path(dirs))? {
            Some(v) => Some(v),
            None => {
                return Err(format!(
                    "backup encryption is on but {} is missing; nothing was written",
                    vault_path(dirs).display()
                ));
            }
        }
    } else {
        None
    };
    Ok(notatnik_graph::BackupOptions {
        compress: dev.ui.backup_compression != "none",
        extras: vec![
            Extra {
                name: "settings.toml".into(),
                path: dirs.config.join("settings.toml"),
                tracked: true,
            },
            Extra {
                name: "state.json".into(),
                path: dirs.graph_data(graph_id).join("state.json"),
                tracked: false,
            },
        ],
        seal_to,
        keys,
        graph_keys: None,
    })
}

/// One backup run with the device settings as they are now: a snapshot (when `force`, or when one is due: the
/// newest is older than `min_age` and something changed), then the copies in the second folder. The second folder
/// is brought up to date even when no snapshot was due, so a disk that was away catches up. Returns the outcome of
/// the snapshot and the error of the copying, if any.
fn run_backup(
    dirs: &Dirs,
    graph: &Graph,
    force: bool,
    min_age: Option<chrono::Duration>,
    keys: Option<Arc<notatnik_crypt::Keys>>,
) -> (
    notatnik_graph::Result<Option<notatnik_graph::BackupFile>>,
    Option<String>,
) {
    let dev = notatnik_graph::DeviceSettings::load(dirs).unwrap_or_default();
    let dir = primary_dir(dirs, &dev);
    let opts = match backup_options(dirs, &dev, &graph.id(), keys, true) {
        Ok(o) => o,
        Err(m) => return (Err(GraphError::Crypt(m)), None),
    };
    let made = graph.backup_now(
        &dir,
        chrono::Local::now().naive_local(),
        force,
        min_age,
        &opts,
    );
    let mirror_error = mirror_dir(&dev).and_then(|m| {
        notatnik_graph::sync_mirror(&dir, &m, &graph.id())
            .err()
            .map(|e| e.to_string())
    });
    match &made {
        Ok(Some(f)) => log::info!(
            "backup {} written ({} KiB{})",
            f.name,
            f.bytes / 1024,
            if f.encrypted { ", encrypted" } else { "" }
        ),
        Ok(None) => log::debug!("backup: nothing to do (no change, or the last one is recent)"),
        Err(e) => log::warn!("backup failed: {e}"),
    }
    if let Some(e) = &mirror_error {
        log::warn!("backup: the second folder could not be brought up to date: {e}");
    }
    (made, mirror_error)
}

impl Drop for BackupTask {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

mod calendar;
mod conflicts;
mod encryption;
mod favorites;
mod filesources;
mod hidden;
pub mod slots;
mod syncing;
mod transfer;
pub use syncing::{MemorySecrets, Secrets};

struct Open {
    /// Keeps other programs from opening this graph; released when the graph is closed or the program ends.
    _lock: Option<notatnik_graph::GraphLock>,
    graph: Graph,
    _watcher: Option<Watcher>,
    backup: Option<BackupTask>,
    /// The sync with a server folder, for a graph that has one.
    sync: Option<syncing::SyncTask>,
    trackers: HashMap<String, BlockIdentity>,
    undo: Vec<UndoEntry>,
    redo: Vec<UndoEntry>,
    ui: UiState,
    state_path: PathBuf,
    warnings: Vec<String>,
}

/// The application: one open graph per state. The first state also holds the other open graphs (`slots.rs`).
pub struct AppState {
    inner: Mutex<Option<Open>>,
    emit: Emitter,
    today: Arc<dyn Fn() -> NaiveDate + Send + Sync>,
    dirs: Dirs,
    watch: bool,
    /// The plan of a rename that stopped half way (for `rename_revert`).
    last_rename: Mutex<Option<notatnik_graph::RenamePlan>>,
    /// The unlocked backup vault (EN-2), if any. One for the device, so every slot shares it (MG-1).
    backup_keys: Arc<Mutex<Option<Unlocked>>>,
    /// Passwords of server logins.
    secrets: Arc<dyn Secrets>,
    /// The unlocked key of the open graph's vault (EN-13, EN-14), shared with the sync thread.
    graph_keys: encryption::SharedKeys,
    /// A graph vault that was made and not confirmed yet.
    graph_pending: Mutex<Option<encryption::PendingVault>>,
    /// A calendar server to use instead of the real one (tests).
    calendar_client: Option<Arc<dyn notatnik_caldav::CalDavClient + Send + Sync>>,
    /// Which slot this state is: `""` for the root (the first graph), otherwise the name it was opened under (MG-1).
    slot: String,
    /// One transfer between graphs at a time (it holds two graphs, so two at once could wait for each other).
    transfer_lock: Mutex<()>,
    /// What the last transfer did, for its undo (MG-4).
    last_transfer: Mutex<Option<transfer::TransferUndo>>,
    /// The other open graphs by slot. Only the root has any: it routes the commands that name a slot.
    slots: Mutex<HashMap<String, Arc<AppState>>>,
    /// The folder of the graph open in each slot (all states share this list), so the same graph is not opened twice.
    open_roots: Arc<Mutex<HashMap<String, PathBuf>>>,
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
            last_rename: Mutex::new(None),
            backup_keys: Arc::default(),
            secrets: Arc::new(MemorySecrets::default()),
            graph_keys: Arc::default(),
            graph_pending: Mutex::new(None),
            calendar_client: None,
            slot: String::new(),
            transfer_lock: Mutex::new(()),
            last_transfer: Mutex::new(None),
            slots: Mutex::new(HashMap::new()),
            open_roots: Arc::default(),
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

    /// The lock of the graph folder `path`: the one this state holds already when it is the same folder, else a new one.
    fn lock_graph(&self, path: &Path) -> ApiResult<notatnik_graph::GraphLock> {
        let want = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        let kept = self
            .lock()
            .as_mut()
            .and_then(|o| o._lock.take_if(|l| l.root() == want));
        if let Some(l) = kept {
            return Ok(l);
        }
        notatnik_graph::GraphLock::acquire(&want).map_err(|e| match e {
            notatnik_graph::LockError::Held(_) => ApiError::new("locked", e.to_string()),
            notatnik_graph::LockError::Io(_) => ApiError::new("internal", e.to_string()),
        })
    }

    /// `open_graph`
    pub fn open_graph(&self, a: OpenGraphArgs) -> ApiResult<GraphInfo> {
        let path = PathBuf::from(&a.path);
        self.refuse_if_open_elsewhere(&path)?;
        // Where the app runs for real (the watcher is on), another program with the same graph open is refused: two
        // sync loops and two writers on one folder would corrupt it. Opening the graph this state has open again keeps
        // its lock.
        let lock = if self.watch {
            Some(self.lock_graph(&path)?)
        } else {
            None
        };
        let started = std::time::Instant::now();
        let (graph, report) =
            if a.create && !path.join(".notatnik").exists() && !path.join("pages").exists() {
                Graph::create(&path, &self.dirs)?
            } else {
                Graph::open(&path, &self.dirs)?
            };
        Self::log_opened(&graph, &report, started.elapsed());
        let emit = self.emit.clone();
        // a change on this device asks for a sync a few seconds later (SY-6)
        let wake = Arc::new(syncing::Wake::default());
        let kick = wake.clone();
        let watcher = if self.watch {
            Some(graph.watch_with(Duration::from_millis(200), move |ev| {
                log::debug!("files changed: {}", ev.paths.join(", "));
                kick.kick(syncing::SYNC_AFTER_WRITE_SECS);
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
        let mut info = Self::info(&graph, report.warnings.clone());
        info.remote = self.remote_of(&graph.id()).map(|r| RemoteInfo {
            url: r.url,
            user: r.user,
        });
        info.color = self.graph_color(&graph.id());
        // the history of graphs: this one is now the last opened (opened by the app only, not by the CLI)
        let mut dev = notatnik_graph::DeviceSettings::load(&self.dirs).unwrap_or_default();
        let last = dev.last_graph.clone();
        dev.register(&graph.id(), graph.root());
        if !self.slot.is_empty() {
            // a graph opened beside the first one is not the one the app starts with (MG-1)
            dev.last_graph = last;
        }
        let _ = dev.save(&self.dirs);
        // snapshots run on a thread of their own, only where the file watcher runs too (not in unit tests)
        let backup = self.watch.then(|| self.start_backups(&graph));
        self.unlock_from_keyring(&graph);
        let sync = self.start_sync(&graph, wake);
        self.note_open_root(&graph);
        *self.lock() = Some(Open {
            _lock: lock,
            graph,
            _watcher: watcher,
            backup,
            sync,
            trackers: HashMap::new(),
            undo: Vec::new(),
            redo: Vec::new(),
            ui,
            state_path,
            warnings: report.warnings,
        });
        Ok(info)
    }

    /// What opening a graph did, for the log: how the index was brought up to date, and every warning.
    fn log_opened(graph: &Graph, report: &notatnik_graph::OpenReport, took: Duration) {
        let pages = graph.with_index(|i| i.counts()).map_or(0, |c| c.pages);
        let r = &report.reconcile;
        let how = if report.index_rebuilt {
            "index rebuilt".to_string()
        } else {
            format!(
                "index updated: {} new, {} changed, {} gone",
                r.added,
                r.reindexed - r.added.min(r.reindexed),
                r.deleted
            )
        };
        log::info!(
            "opened graph {} ({}): {pages} pages, {how}, {} ms",
            graph.root().display(),
            graph.id(),
            took.as_millis()
        );
        for w in &report.warnings {
            log::warn!("graph {}: {w}", graph.id());
        }
        for r in &report.reids {
            log::warn!("duplicate block ID in {} was given a new one", r.path);
        }
        for c in graph.conflict_files().unwrap_or_default() {
            log::warn!("conflict copy waiting to be merged by hand: {c}");
        }
    }

    fn history(&self) -> Vec<GraphHistoryEntry> {
        let dev = notatnik_graph::DeviceSettings::load(&self.dirs).unwrap_or_default();
        let last = dev.last_graph.clone();
        // the most recently opened first
        dev.graphs
            .iter()
            .rev()
            .map(|g| {
                let p = Path::new(&g.path);
                GraphHistoryEntry {
                    id: g.id.clone(),
                    name: p
                        .file_name()
                        .map_or_else(|| g.path.clone(), |n| n.to_string_lossy().into_owned()),
                    path: g.path.clone(),
                    exists: p.is_dir(),
                    last: last.as_deref() == Some(g.id.as_str()),
                    remote_url: g.remote.as_ref().map(|r| r.url.clone()),
                    color: dev.color_of(&g.id),
                }
            })
            .collect()
    }

    /// `list_graphs`: the graphs opened on this device, the most recent first (works without an open graph).
    pub fn list_graphs(&self, _a: NoArgs) -> ApiResult<Vec<GraphHistoryEntry>> {
        Ok(self.history())
    }

    /// The color of a graph on this device (MG-5).
    pub(super) fn graph_color(&self, id: &str) -> u8 {
        notatnik_graph::DeviceSettings::load(&self.dirs)
            .unwrap_or_default()
            .color_of(id)
    }

    /// `graph_color_set`: gives a graph a color of the palette, or (`null`) the one its ID gives it again.
    pub fn graph_color_set(&self, a: GraphColorArgs) -> ApiResult<Vec<GraphHistoryEntry>> {
        let mut dev = notatnik_graph::DeviceSettings::load(&self.dirs).unwrap_or_default();
        let color = a.color.map(u8::try_from).transpose().ok().flatten();
        if a.color.is_some() && color.is_none() || !dev.set_color(&a.id, color) {
            return Err(ApiError::new("invalid", "that color is not in the palette"));
        }
        dev.save(&self.dirs)?;
        Ok(self.history())
    }

    /// `forget_graph`: removes a graph from the history. Its folder, its cache and its backups are not touched.
    pub fn forget_graph(&self, a: ForgetGraphArgs) -> ApiResult<Vec<GraphHistoryEntry>> {
        self.forget_remote_secret(&a.id);
        self.forget_graph_key(&a.id);
        let mut dev = notatnik_graph::DeviceSettings::load(&self.dirs).unwrap_or_default();
        dev.forget(&a.id);
        dev.save(&self.dirs)?;
        Ok(self.history())
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
            completed_log_limit: graph.config().tasks.completed_log_limit,
            priority_labels: graph.config().priority_labels(),
            remote: None,
            color: 0,
        }
    }

    /// `graph_info`
    pub fn graph_info(&self, _a: NoArgs) -> ApiResult<GraphInfo> {
        self.with_open(|o| {
            let mut info = Self::info(&o.graph, o.warnings.clone());
            info.remote = self.remote_of(&o.graph.id()).map(|r| RemoteInfo {
                url: r.url,
                user: r.user,
            });
            info.color = self.graph_color(&o.graph.id());
            Ok(info)
        })
    }

    /// `today`
    pub fn today_cmd(&self, _a: NoArgs) -> ApiResult<StringValue> {
        Ok(StringValue {
            value: self.today().format("%Y-%m-%d").to_string(),
        })
    }

    fn build_view(o: &mut Open, name: &PageName) -> ApiResult<(PageView, String)> {
        let info = o.graph.with_index(|i| i.page_info(name.as_str()))?;
        let (mut canonical, rel, _indexed) = match &info {
            Some(i) => {
                let n = PageName::parse(&i.name).unwrap_or_else(|_| name.clone());
                let rel = i.file_path.clone().unwrap_or_else(|| page_to_path(&n));
                (n, rel, i.file_path.is_some() && !i.is_virtual)
            }
            None => (name.clone(), page_to_path(name), false),
        };
        // trust the file on disk over the index: it may have been created or changed a moment ago
        let file = o.graph.read_page(&rel).ok();
        if let Some(f) = &file {
            if o.graph
                .refresh_file(&rel, &f.hash)
                .map_err(ApiError::from)?
            {
                if let Some(i2) = o.graph.with_index(|i| i.page_info(name.as_str()))? {
                    canonical = PageName::parse(&i2.name).unwrap_or(canonical);
                }
            }
        }
        let (text, hash, exists) = match file {
            Some(f) => (f.text, Some(f.hash), true),
            None => (String::new(), None, false),
        };
        let cfg = o.graph.config();
        let parsed = parse_page_with(&text, &cfg.parse_options());
        let hidden: HashSet<String> = cfg.properties.hidden.iter().cloned().collect();
        let followed = o
            .graph
            .with_index(|i| i.followed_uids())
            .unwrap_or_default();
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
                followed: &followed,
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

    /// `page_aliases`: sets the aliases of a page (LN-6): other names that `[[links]]` can use, that search and the
    /// `[[` list find, and whose links count as backlinks of the page. An alias that is already the name (or an
    /// alias) of another page is refused, so a link never means two pages.
    pub fn page_aliases(&self, a: PageAliasesArgs) -> ApiResult<PageView> {
        let name = Self::parse_name(&a.page)?;
        let mut aliases: Vec<PageName> = Vec::new();
        for raw in &a.aliases {
            let raw = raw.trim();
            if raw.is_empty() {
                continue;
            }
            let n = Self::parse_name(raw)?;
            // a name only differing in case is the same alias; the page's own name is no alias
            if n.lookup_key() == name.lookup_key()
                || aliases.iter().any(|x| x.lookup_key() == n.lookup_key())
            {
                continue;
            }
            aliases.push(n);
        }
        let (view, had) = self.with_open(|o| {
            let (view, _) = Self::build_view(o, &name)?;
            for n in &aliases {
                let other = o
                    .graph
                    .with_index(|i| i.page_info(n.as_str()))?
                    .filter(|i| !i.is_virtual && !i.name.eq_ignore_ascii_case(&view.name));
                if let Some(other) = other {
                    return Err(ApiError::new(
                        "exists",
                        format!("`{n}` is already the name of the page `{}`", other.name),
                    ));
                }
            }
            let had = view
                .frontmatter
                .as_ref()
                .is_some_and(|f| !f.aliases.is_empty());
            Ok((view, had))
        })?;
        let ops = if !aliases.is_empty() {
            let items: Vec<String> = aliases.iter().map(|n| yaml_item(n.as_str())).collect();
            vec![BlockOp::SetPageProperty {
                key: "aliases".into(),
                value: format!("[{}]", items.join(", ")),
            }]
        } else if had {
            vec![BlockOp::RemovePageProperty {
                key: "aliases".into(),
            }]
        } else {
            return Ok(view);
        };
        Ok(self
            .apply_ops(ApplyOpsArgs {
                page: view.name.clone(),
                base_hash: view.hash.clone(),
                ops,
            })?
            .page)
    }

    /// `apply_ops`
    pub fn apply_ops(&self, a: ApplyOpsArgs) -> ApiResult<ApplyResult> {
        self.apply_ops_with(a, None)
    }

    /// `apply_ops`; `force_type` applies the template of that type even when the ops do not set `type`.
    fn apply_ops_with(
        &self,
        a: ApplyOpsArgs,
        force_type: Option<String>,
    ) -> ApiResult<ApplyResult> {
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
            // setting `type` also adds the template's missing properties, in the same edit (one undo step)
            let mut ops = a.ops.clone();
            let mut applied = (None, Vec::new());
            let mut template_error: Option<String> = None;
            let set_type = ops.iter().find_map(|op| match op {
                BlockOp::SetPageProperty { key, value } if key.eq_ignore_ascii_case("type") => {
                    templates::type_name(value)
                }
                _ => None,
            });
            let auto = force_type.is_some() || self.templates_auto();
            let today = self.today();
            // a new page below a namespace (or a new journal day) starts from that template (NS-10, JR-6)
            if created && auto {
                if let Some(ty) = Self::creation_template(o, &canonical, &rel) {
                    let r = Self::template_ops(
                        o,
                        &canonical,
                        &rel,
                        false,
                        &ty,
                        &ops,
                        today,
                        TemplateMode::NewPage,
                    );
                    if let Some(t) = r.unwrap_or_else(|e| {
                        template_error = Some(e.message);
                        None
                    }) {
                        // the user's first blocks come after the template's
                        for op in &mut ops {
                            if let BlockOp::Insert { index, .. } = op {
                                *index += t.top_level;
                            }
                        }
                        let mut all = t.ops;
                        all.append(&mut ops);
                        ops = all;
                        applied = (Some(t.file), t.added);
                    }
                }
            }
            if let Some(ty) = force_type.or(set_type).filter(|_| auto) {
                let r = Self::template_ops(
                    o,
                    &canonical,
                    &rel,
                    !created,
                    &ty,
                    &ops,
                    today,
                    TemplateMode::Type,
                );
                if let Some(t) = r.unwrap_or_else(|e| {
                    template_error = Some(e.message);
                    None
                }) {
                    ops.extend(t.ops);
                    if applied.0.is_none() {
                        applied = (Some(t.file), t.added);
                    } else {
                        applied.1.extend(t.added);
                    }
                }
            }
            let out = o.graph.edit_page(&rel, a.base_hash.as_deref(), &ops)?;
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
            (self.emit)(
                "graph-changed",
                json!({ "pages": [canonical.to_string()], "paths": [], "own": true }),
            );
            Ok(ApplyResult {
                page,
                template: applied.0,
                template_added: applied.1,
                template_error,
            })
        })
    }

    fn templates_auto(&self) -> bool {
        notatnik_graph::DeviceSettings::load(&self.dirs)
            .unwrap_or_default()
            .ui
            .templates_auto
    }

    /// The template file of a type: `templates/<type>.md`, the name compared without regard to case. A snippet is
    /// `snippets/<name>`, in `templates/snippets/`; the returned file name keeps that prefix.
    fn template_text(o: &Open, ty: &str) -> Option<(String, String)> {
        let (dir, prefix, leaf) = match ty.strip_prefix("snippets/") {
            Some(leaf) => ("templates/snippets", "snippets/", leaf),
            None => ("templates", "", ty),
        };
        let file = std::fs::read_dir(o.graph.root().join(dir))
            .ok()?
            .flatten()
            .find_map(|e| {
                let n = e.file_name().into_string().ok()?;
                let stem = n.strip_suffix(".md")?;
                stem.eq_ignore_ascii_case(leaf).then(|| stem.to_string())
            })?;
        let text = o.graph.read_page(&format!("{dir}/{file}.md")).ok()?.text;
        Some((format!("{prefix}{file}"), text))
    }

    /// The template for a page that is being created: the journal template for a journal day, otherwise the one of
    /// the longest configured namespace the page is below. Returns the template name.
    fn creation_template(o: &Open, page: &PageName, rel: &str) -> Option<String> {
        let cfg = o.graph.config();
        if rel.starts_with("journals/") {
            return cfg
                .graph
                .journal_template
                .as_deref()
                .and_then(templates::template_name);
        }
        let lower = page.as_str().to_lowercase();
        cfg.namespaces
            .iter()
            .filter_map(|(ns, sec)| {
                let ns = ns.trim().trim_matches('/').to_lowercase();
                let below = !ns.is_empty()
                    && lower.len() > ns.len()
                    && lower.starts_with(&ns)
                    && lower.as_bytes()[ns.len()] == b'/';
                below.then_some((ns.len(), sec.template.as_deref()))
            })
            .max_by_key(|(len, _)| *len)
            .and_then(|(_, t)| t.and_then(templates::template_name))
    }

    /// What the template of type `ty` adds to the page (`None` without a template). `Type`: the properties the page
    /// lacks, and the blocks of a page without any. `NewPage`: also the template's own `type`, and the blocks when
    /// the edit only inserts top-level blocks (they are moved after the template's).
    #[allow(clippy::too_many_arguments)]
    fn template_ops(
        o: &Open,
        page: &PageName,
        rel: &str,
        exists: bool,
        ty: &str,
        pending: &[BlockOp],
        today: NaiveDate,
        mode: TemplateMode,
    ) -> ApiResult<Option<TemplateOps>> {
        let Some((file, text)) = Self::template_text(o, ty) else {
            return Ok(None);
        };
        // nothing is inserted from a template that cannot be read (no partial insert)
        if notatnik_core::parse_page(&text)
            .frontmatter
            .is_some_and(|f| !f.valid)
        {
            return Err(ApiError::new(
                "invalid",
                format!("the template `templates/{file}.md` has invalid frontmatter; fix it first"),
            ));
        }
        // a journal day: `{{date}}` is that day
        let date = if rel.starts_with("journals/") {
            page.as_str().to_string()
        } else {
            today.format("%Y-%m-%d").to_string()
        };
        // the page's own properties, with the ones this very edit sets (the `type` that picked the template)
        let mut page_props = if exists {
            o.graph
                .read_page(rel)
                .map(|p| templates::page_props(&p.text))
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        for op in pending {
            if let BlockOp::SetPageProperty { key, value } = op {
                page_props.retain(|(k, _)| *k != key.to_lowercase());
                page_props.push((key.to_lowercase(), value.clone()));
            }
        }
        let t = templates::parse_template(
            &text,
            &templates::Vars {
                title: page.as_str(),
                date: &date,
                time: &templates::now_time(),
                props: &page_props,
                selection: "",
                clipboard: "",
                answers: &[],
            },
        );
        let mut have: Vec<String> = pending
            .iter()
            .filter_map(|op| match op {
                BlockOp::SetPageProperty { key, .. } => Some(key.to_lowercase()),
                _ => None,
            })
            .collect();
        let mut no_blocks = true;
        if exists {
            if let Ok(f) = o.graph.read_page(rel) {
                let p = notatnik_core::parse_page(&f.text);
                no_blocks = p.blocks.is_empty();
                if let Some(fm) = p.frontmatter {
                    have.extend(fm.entries.iter().map(|e| e.key.to_lowercase()));
                }
            }
        }
        let with_blocks = match mode {
            TemplateMode::Type => {
                no_blocks
                    && !pending
                        .iter()
                        .any(|op| matches!(op, BlockOp::Insert { .. }))
            }
            TemplateMode::NewPage => pending.iter().all(|op| {
                matches!(op, BlockOp::SetPageProperty { .. })
                    || matches!(op, BlockOp::Insert { parent, .. } if parent.is_empty())
            }),
        };
        let (ops, added) =
            templates::ops_for(&t, &have, with_blocks, mode == TemplateMode::NewPage);
        let top_level = if with_blocks {
            u32::try_from(t.blocks.iter().filter(|b| b.0.is_empty()).count()).unwrap_or(0) as usize
        } else {
            0
        };
        Ok(Some(TemplateOps {
            file,
            ops,
            added,
            top_level,
        }))
    }

    /// `template_info`: which template the page's `type` selects and what of it is missing.
    pub fn template_info(&self, a: PageArgs) -> ApiResult<TemplateInfo> {
        let name = Self::parse_name(&a.page)?;
        self.with_open(|o| {
            let rel = Self::page_rel(o, name.as_str())?;
            let Ok(f) = o.graph.read_page(&rel) else {
                return Ok(TemplateInfo {
                    template: None,
                    missing: Vec::new(),
                });
            };
            let fm = notatnik_core::parse_page(&f.text).frontmatter;
            let ty = fm
                .as_ref()
                .and_then(|fm| {
                    fm.entries
                        .iter()
                        .find(|e| e.key.eq_ignore_ascii_case("type"))
                })
                .and_then(|e| templates::type_name(&e.raw));
            let found = ty.and_then(|ty| {
                Self::template_ops(
                    o,
                    &name,
                    &rel,
                    true,
                    &ty,
                    &[],
                    self.today(),
                    TemplateMode::Type,
                )
                .ok()
                .flatten()
            });
            Ok(match found {
                Some(t) => TemplateInfo {
                    template: Some(t.file),
                    missing: t.added,
                },
                None => TemplateInfo {
                    template: None,
                    missing: Vec::new(),
                },
            })
        })
    }

    fn template_entry(o: &Open, name: &str, text: &str) -> TemplateEntry {
        let _ = o;
        let t = templates::parse_template(
            text,
            &templates::Vars {
                title: "",
                date: "",
                time: "",
                props: &[],
                selection: "",
                clipboard: "",
                answers: &[],
            },
        );
        TemplateEntry {
            name: name.to_string(),
            fields: t.props.into_iter().map(|p| p.0).collect(),
            blocks: u32::try_from(t.blocks.len()).unwrap_or(u32::MAX),
        }
    }

    /// `list_templates`
    pub fn list_templates(&self, _a: NoArgs) -> ApiResult<Vec<TemplateEntry>> {
        self.with_open(|o| Ok(Self::templates_in(o)))
    }

    fn templates_in(o: &Open) -> Vec<TemplateEntry> {
        let mut out: Vec<TemplateEntry> = std::fs::read_dir(o.graph.root().join("templates"))
            .map(|rd| {
                rd.flatten()
                    .filter_map(|e| {
                        let n = e.file_name().into_string().ok()?;
                        let stem = n.strip_suffix(".md")?.to_string();
                        let text = o.graph.read_page(&format!("templates/{n}")).ok()?.text;
                        Some(Self::template_entry(o, &stem, &text))
                    })
                    .collect()
            })
            .unwrap_or_default();
        out.sort_by_key(|t| t.name.to_lowercase());
        out
    }

    /// `deleted_templates`: templates in the trash that can be restored.
    pub fn deleted_templates(&self, _a: NoArgs) -> ApiResult<Vec<DeletedTemplate>> {
        self.with_open(|o| Ok(Self::deleted_in(o)))
    }

    fn deleted_in(o: &Open) -> Vec<DeletedTemplate> {
        o.graph
            .deleted_templates()
            .into_iter()
            .map(|(id, name, ms)| DeletedTemplate {
                id,
                name,
                #[allow(clippy::cast_precision_loss)]
                deleted_ms: ms as f64,
            })
            .collect()
    }

    /// `restore_template`
    pub fn restore_template(&self, a: RestoreTemplateArgs) -> ApiResult<Vec<TemplateEntry>> {
        self.with_open(|o| {
            o.graph
                .restore_template(&a.id, &a.name)
                .map_err(|e| match e {
                    GraphError::Conflict { .. } => ApiError::new(
                        "exists",
                        format!(
                            "a template `{}` exists now; delete or rename it first",
                            a.name
                        ),
                    ),
                    e => e.into(),
                })?;
            Ok(Self::templates_in(o))
        })
    }

    /// `template_blocks`: the blocks of a template with the variables filled in for `page`, to insert at the cursor
    /// (`/template`).
    pub fn template_blocks(&self, a: TemplateBlocksArgs) -> ApiResult<TemplateContent> {
        let page = Self::parse_name(&a.page)?;
        self.with_open(|o| {
            let (_, text) = Self::template_text(o, &a.name).ok_or_else(|| {
                ApiError::new("not_found", format!("there is no template `{}`", a.name))
            })?;
            let date = self.today().format("%Y-%m-%d").to_string();
            // the page properties the template has and the page lacks
            let rel = Self::page_rel(o, page.as_str())?;
            let exists = o.graph.read_page(&rel).is_ok();
            let page_props = if exists {
                o.graph
                    .read_page(&rel)
                    .map(|p| templates::page_props(&p.text))
                    .unwrap_or_default()
            } else {
                Vec::new()
            };
            let answers: Vec<(String, String)> = a
                .answers
                .iter()
                .flatten()
                .map(|x| (x.label.clone(), x.value.clone()))
                .collect();
            let t = templates::parse_template(
                &text,
                &templates::Vars {
                    title: page.as_str(),
                    date: &date,
                    time: &templates::now_time(),
                    props: &page_props,
                    selection: a.selection.as_deref().unwrap_or(""),
                    clipboard: a.clipboard.as_deref().unwrap_or(""),
                    answers: &answers,
                },
            );
            // a snippet has no page properties
            let props = if a.name.starts_with("snippets/") {
                Vec::new()
            } else {
                Self::template_ops(
                    o,
                    &page,
                    &rel,
                    exists,
                    &a.name,
                    &[],
                    self.today(),
                    TemplateMode::NewPage,
                )?
                .map(|t| {
                    t.ops
                        .into_iter()
                        .filter(|op| matches!(op, BlockOp::SetPageProperty { .. }))
                        .collect()
                })
                .unwrap_or_default()
            };
            let blocks = t
                .blocks
                .into_iter()
                .map(|(parent, index, text)| TemplateBlockView {
                    parent: parent
                        .into_iter()
                        .map(|p| u32::try_from(p).unwrap_or(u32::MAX))
                        .collect(),
                    index: u32::try_from(index).unwrap_or(u32::MAX),
                    text,
                })
                .collect();
            let cursor = t.cursor.map(|(block, offset)| TemplateCursor {
                block: u32::try_from(block).unwrap_or(u32::MAX),
                offset: u32::try_from(offset).unwrap_or(u32::MAX),
            });
            Ok(TemplateContent {
                blocks,
                props,
                cursor,
                uses_clipboard: text.to_lowercase().contains("{{clipboard}}"),
                asks: templates::asks(&text)
                    .into_iter()
                    .map(|q| TemplateAsk {
                        label: q.label,
                        default: q.default,
                    })
                    .collect(),
            })
        })
    }

    /// `delete_template`
    pub fn delete_template(&self, a: NameArgs) -> ApiResult<Vec<TemplateEntry>> {
        self.with_open(|o| {
            o.graph.delete_template(&a.name)?;
            Ok(Self::templates_in(o))
        })
    }

    /// `read_template`: the text of `templates/<name>.md`, as it is in the file.
    pub fn read_template(&self, a: NameArgs) -> ApiResult<StringValue> {
        self.with_open(|o| {
            let (_, text) = Self::template_text(o, &a.name).ok_or_else(|| {
                ApiError::new("not_found", format!("there is no template `{}`", a.name))
            })?;
            Ok(StringValue { value: text })
        })
    }

    /// `create_template`: a new template for a page type, with the `type` line and nothing else. The name is
    /// compared without regard to case (`Person` and `person` are one template).
    pub fn create_template(&self, a: NameArgs) -> ApiResult<TemplateEntry> {
        let name = a.name.trim();
        if name.is_empty() {
            return Err(ApiError::new("invalid", "give the page type a name"));
        }
        if name.contains(['/', '\\']) {
            return Err(ApiError::new(
                "invalid",
                "a page type has no `/` in its name",
            ));
        }
        self.with_open(|o| {
            if Self::template_text(o, name).is_some() {
                return Err(ApiError::new(
                    "exists",
                    format!("the template `{name}` exists already"),
                ));
            }
            // a name with characters that mean something in YAML is quoted
            let plain = name
                .chars()
                .all(|c| c.is_alphanumeric() || matches!(c, ' ' | '_' | '-' | '.'));
            let value = if plain {
                name.to_string()
            } else {
                format!("\"{}\"", name.replace('\\', "\\\\").replace('"', "\\\""))
            };
            let text = format!("---\ntype: {value}\n---\n");
            o.graph.write_template(name, &text)?;
            Ok(Self::template_entry(o, name, &text))
        })
    }

    /// `link_title`: the title of a web page, for a pasted address (ED-27). Refused while the device setting
    /// `paste_titles` is off, so the app never contacts a site the user did not allow. An empty value: no title.
    pub fn link_title(&self, a: UrlArgs) -> ApiResult<StringValue> {
        if !notatnik_graph::DeviceSettings::load(&self.dirs)?
            .ui
            .paste_titles
        {
            return Err(ApiError::new(
                "disabled",
                "looking up titles is off (Settings → Appearance)",
            ));
        }
        let value = crate::linktitle::fetch_title(a.url.trim())
            .map_err(|m| ApiError::new("unreachable", m))?
            .unwrap_or_default();
        Ok(StringValue { value })
    }

    /// `list_snippets`: the snippets in `templates/snippets/`, by their plain names (the `snippets/` prefix is for
    /// the other template calls). `fields` is always empty.
    pub fn list_snippets(&self, _a: NoArgs) -> ApiResult<Vec<TemplateEntry>> {
        self.with_open(|o| {
            let mut out: Vec<TemplateEntry> =
                std::fs::read_dir(o.graph.root().join("templates/snippets"))
                    .map(|rd| {
                        rd.flatten()
                            .filter_map(|e| {
                                let n = e.file_name().into_string().ok()?;
                                let stem = n.strip_suffix(".md")?.to_string();
                                let text = o
                                    .graph
                                    .read_page(&format!("templates/snippets/{n}"))
                                    .ok()?
                                    .text;
                                Some(Self::template_entry(o, &stem, &text))
                            })
                            .collect()
                    })
                    .unwrap_or_default();
            out.sort_by_key(|t| t.name.to_lowercase());
            Ok(out)
        })
    }

    /// `save_snippet`: the blocks of `text` become `templates/snippets/<name>.md` (PR-10). Returns the entry under
    /// its plain name.
    pub fn save_snippet(&self, a: SaveSnippetArgs) -> ApiResult<TemplateEntry> {
        let name = a.name.trim();
        if name.is_empty() {
            return Err(ApiError::new("invalid", "give the template a name"));
        }
        if name.contains(['/', '\\', '\0']) || name.starts_with('.') {
            return Err(ApiError::new(
                "invalid",
                "a template name has no `/` and does not start with a dot",
            ));
        }
        let mut text = a.text.replace("\r\n", "\n");
        if !text.ends_with('\n') {
            text.push('\n');
        }
        if notatnik_core::parse_page(&text).blocks.is_empty() {
            return Err(ApiError::new("invalid", "there are no blocks to save"));
        }
        self.with_open(|o| {
            let key = format!("snippets/{name}");
            // an existing snippet keeps its file name when it is replaced (`Notes.md` stays `Notes.md`)
            let file = match Self::template_text(o, &key) {
                Some(_) if !a.replace => {
                    return Err(ApiError::new(
                        "exists",
                        format!("the template `{name}` exists already"),
                    ));
                }
                Some((f, _)) => f,
                None => key,
            };
            o.graph.write_template(&file, &text)?;
            let mut entry = Self::template_entry(o, &file, &text);
            entry.name = file.trim_start_matches("snippets/").to_string();
            Ok(entry)
        })
    }

    /// `write_template`: replaces the text of an existing template. The text it replaces stays in the version
    /// history of the file.
    pub fn write_template(&self, a: WriteTemplateArgs) -> ApiResult<TemplateEntry> {
        self.with_open(|o| {
            let (file, _) = Self::template_text(o, &a.name).ok_or_else(|| {
                ApiError::new("not_found", format!("there is no template `{}`", a.name))
            })?;
            let mut text = a.text.replace("\r\n", "\n");
            if !text.is_empty() && !text.ends_with('\n') {
                text.push('\n');
            }
            o.graph.write_template(&file, &text)?;
            Ok(Self::template_entry(o, &file, &text))
        })
    }

    /// `save_template`: makes `templates/<type>.md` from a page.
    pub fn save_template(&self, a: SaveTemplateArgs) -> ApiResult<TemplateEntry> {
        let name = Self::parse_name(&a.page)?;
        self.with_open(|o| {
            let rel = Self::page_rel(o, name.as_str())?;
            let text = o
                .graph
                .read_page(&rel)
                .map_err(|_| ApiError::new("not_found", "the page has no properties yet"))?
                .text;
            let (ty, body) = templates::render(&text, a.keep_values).ok_or_else(|| {
                ApiError::new(
                    "invalid",
                    "set a `type` on the page first: it names the template",
                )
            })?;
            // an existing template of that type keeps its file name (`Person.md` stays `Person.md`)
            let file = Self::template_text(o, &ty).map_or(ty, |(f, _)| f);
            if !a.replace && Self::template_text(o, &file).is_some() {
                return Err(ApiError::new(
                    "exists",
                    format!("the template `{file}` exists already"),
                ));
            }
            o.graph.write_template(&file, &body)?;
            Ok(Self::template_entry(o, &file, &body))
        })
    }

    /// `apply_template`: adds the properties of the page's template that the page lacks.
    pub fn apply_template(&self, a: PageArgs) -> ApiResult<ApplyResult> {
        let name = Self::parse_name(&a.page)?;
        let (hash, ty) = self.with_open(|o| {
            let rel = Self::page_rel(o, name.as_str())?;
            let f = o
                .graph
                .read_page(&rel)
                .map_err(|_| ApiError::new("not_found", "the page has no properties yet"))?;
            let ty = notatnik_core::parse_page(&f.text)
                .frontmatter
                .and_then(|fm| {
                    fm.entries
                        .iter()
                        .find(|e| e.key.eq_ignore_ascii_case("type"))
                        .and_then(|e| templates::type_name(&e.raw))
                });
            Ok((f.hash, ty))
        })?;
        let ty = ty.ok_or_else(|| ApiError::new("invalid", "the page has no `type` property"))?;
        let r = self.apply_ops_with(
            ApplyOpsArgs {
                page: a.page,
                base_hash: Some(hash),
                ops: Vec::new(),
            },
            Some(ty.clone()),
        )?;
        if let Some(m) = r.template_error {
            return Err(ApiError::new("invalid", m));
        }
        if r.template.is_none() {
            return Err(ApiError::new(
                "not_found",
                format!("there is no template `templates/{ty}.md`"),
            ));
        }
        Ok(r)
    }

    #[allow(clippy::unused_self)]
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
            (self.emit)(
                "graph-changed",
                json!({ "pages": [entry.page.clone()], "paths": [], "own": true }),
            );
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
            // a mistake in the syntax (`(a b`, `-a`) is explained to the user, not an internal error
            let hits = match o.graph.with_index(|i| i.search(&a.text, a.limit as usize)) {
                Ok(h) => h,
                Err(notatnik_index::Error::Query(m)) => {
                    return Err(ApiError::new("invalid", m));
                }
                Err(e) => return Err(e.into()),
            };
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
            // A reference is shown with its whole subtree (LN-10), so a block below another referencing block of the
            // same page is already part of that one and is left out.
            let mut paths: std::collections::HashSet<(&str, &str)> =
                std::collections::HashSet::new();
            for l in &links {
                if let Some(p) = l.block_path.as_deref() {
                    paths.insert((l.source_page.as_str(), p));
                }
            }
            let nested = |l: &notatnik_index::Backlink| {
                l.block_path.as_deref().is_some_and(|p| {
                    let mut cut = p;
                    while let Some((head, _)) = cut.rsplit_once('.') {
                        if paths.contains(&(l.source_page.as_str(), head)) {
                            return true;
                        }
                        cut = head;
                    }
                    false
                })
            };
            let links: Vec<_> = links.iter().filter(|l| !nested(l)).cloned().collect();
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
                            .cloned()
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

    /// `unlinked_refs`: plain-text mentions of the page (or its aliases) that are not links (LN-4).
    pub fn unlinked_refs(&self, a: PageArgs) -> ApiResult<Vec<UnlinkedView>> {
        self.with_open(|o| {
            let found = o.graph.with_index(|i| i.unlinked_refs(&a.page, 200))?;
            // the target of the new link: the page's real name (a virtual page keeps the name as typed)
            let target = Self::parse_name(&a.page)?.as_str().to_string();
            let mut views: HashMap<String, PageView> = HashMap::new();
            let mut out = Vec::new();
            for u in found {
                let path: Vec<u32> = u
                    .block_path
                    .split('.')
                    .filter_map(|x| x.parse().ok())
                    .collect();
                if !views.contains_key(&u.source_page) {
                    let n = Self::parse_name(&u.source_page)?;
                    views.insert(u.source_page.clone(), Self::build_view(o, &n)?.0);
                }
                let Some(block) = views
                    .get(&u.source_page)
                    .and_then(|v| find_block(&v.blocks, &path))
                else {
                    continue;
                };
                let Some(link_text) =
                    notatnik_core::mentions::link_first_mention(&block.text, &u.term, &target)
                else {
                    continue; // the index was stale: the text no longer mentions it
                };
                let mut block = block.clone();
                block.children = Vec::new();
                out.push(UnlinkedView {
                    source_page: u.source_page,
                    block_path: path,
                    block,
                    breadcrumbs: u.breadcrumbs,
                    term: u.term,
                    link_text,
                });
            }
            Ok(out)
        })
    }

    /// `page_property_keys`: the page properties in use, for the "group by" picker.
    pub fn page_property_keys(&self, _a: NoArgs) -> ApiResult<Vec<PageKeyView>> {
        self.with_open(|o| {
            Ok(o.graph
                .with_index(|i| i.page_property_keys())?
                .into_iter()
                .map(|k| PageKeyView {
                    key: k.key,
                    pages: k.pages as u32,
                })
                .collect())
        })
    }

    /// `page_groups`: pages grouped by the value of a page property.
    pub fn page_groups(&self, a: PageGroupsArgs) -> ApiResult<Vec<PageGroupView>> {
        self.with_open(|o| {
            Ok(o.graph
                .with_index(|i| i.page_groups(&a.key))?
                .into_iter()
                .map(|g| PageGroupView {
                    value: g.value,
                    pages: g.pages,
                })
                .collect())
        })
    }

    /// `subpages`: every page below a page in the namespace tree, at any depth, in tree order (the "Hierarchy" list).
    pub fn subpages(&self, a: PageArgs) -> ApiResult<Vec<TitleHit>> {
        self.with_open(|o| {
            Ok(o.graph
                .with_index(|i| i.subtree_pages(&a.page))?
                .into_iter()
                .map(|p| TitleHit {
                    name: p.name,
                    is_virtual: p.is_virtual,
                })
                .collect())
        })
    }

    /// `page_icons`: the icon of every page that has one. A page's own `icon:` property wins; otherwise the first
    /// matching rule of the graph's `[[page_icons]]` decides.
    pub fn page_icons(&self, _a: NoArgs) -> ApiResult<Vec<PageIconView>> {
        self.with_open(|o| {
            let rules = o.graph.config().page_icons.clone();
            let found = o.graph.with_index(|i| {
                let mut by_rule = Vec::new();
                for r in &rules {
                    by_rule.push((r.icon.clone(), i.pages_with_property(&r.key, &r.value)?));
                }
                Ok::<_, notatnik_index::Error>((by_rule, i.page_property_values("icon")?))
            })?;
            let mut icons: std::collections::BTreeMap<String, (String, String)> =
                std::collections::BTreeMap::new();
            // rules first (the first matching rule wins), then the explicit `icon:` of a page overrides
            for (icon, pages) in found.0 {
                for name in pages {
                    icons
                        .entry(name.to_lowercase())
                        .or_insert((name.clone(), icon.clone()));
                }
            }
            for (name, icon) in found.1 {
                icons.insert(name.to_lowercase(), (name, icon));
            }
            // today's journal may have no file yet, so it has no `type: journal` row: give it the journal rule's
            // icon (unless a file of that name exists and says otherwise)
            if let Some(rule) = rules.iter().find(|r| {
                r.key.eq_ignore_ascii_case("type") && r.value.trim().eq_ignore_ascii_case("journal")
            }) {
                let today = (self.today)().format("%Y-%m-%d").to_string();
                let has_file = o
                    .graph
                    .with_index(|i| i.page_property_values("type"))?
                    .iter()
                    .any(|(n, _)| n == &today);
                if !has_file {
                    icons
                        .entry(today.clone())
                        .or_insert((today, rule.icon.clone()));
                }
            }
            Ok(icons
                .into_values()
                .map(|(name, icon)| PageIconView { name, icon })
                .collect())
        })
    }

    /// `segment_view`: every place where a name occurs as a path segment, with the pages below it (the
    /// "implicit tag" view of `Renewal` in `P/EY/Renewal/APotheke`).
    pub fn segment_view(&self, a: SegmentArgs) -> ApiResult<SegmentView> {
        self.with_open(|o| {
            let groups = o.graph.with_index(|i| i.segment_groups(&a.segment))?;
            Ok(SegmentView {
                segment: a.segment,
                groups: groups
                    .into_iter()
                    .map(|g| SegmentGroupView {
                        occurrence: g.occurrence,
                        occurrence_is_virtual: g.occurrence_is_virtual,
                        pages: g
                            .pages
                            .into_iter()
                            .map(|(name, is_virtual)| TitleHit { name, is_virtual })
                            .collect(),
                    })
                    .collect(),
            })
        })
    }

    /// `segment_places`: in how many places each segment occurs (for the badge in the hierarchy).
    pub fn segment_places(&self, a: SegmentPlacesArgs) -> ApiResult<SegmentPlacesView> {
        self.with_open(|o| {
            let n = o.graph.with_index(|i| i.segment_places(&a.segments))?;
            Ok(SegmentPlacesView {
                places: n.into_iter().map(|x| x as u32).collect(),
            })
        })
    }

    /// `page_files`: the file of each page name, for marks such as the lock of an encrypted page.
    pub fn page_files(&self, a: PageFilesArgs) -> ApiResult<PageFilesView> {
        self.with_open(|o| {
            Ok(PageFilesView {
                files: o.graph.with_index(|i| i.files_of_pages(&a.names))?,
            })
        })
    }

    /// `all_pages`: every page with counts, for the all-pages view.
    pub fn all_pages(&self, _a: NoArgs) -> ApiResult<Vec<PageListRow>> {
        self.with_open(|o| {
            let rows = o.graph.with_index(|i| i.all_pages())?;
            Ok(rows
                .into_iter()
                .map(|r| PageListRow {
                    path: r.file_path,
                    name: r.name,
                    is_virtual: r.is_virtual,
                    is_journal: r.is_journal,
                    blocks: r.blocks as u32,
                    backlinks: r.backlinks as u32,
                    modified: r.mtime_ns.and_then(|ns| {
                        chrono::DateTime::from_timestamp(ns.div_euclid(1_000_000_000), 0)
                            .map(|d| d.format("%Y-%m-%d %H:%M").to_string())
                    }),
                })
                .collect())
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
                    path: r.file_path,
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
            let p = o
                .graph
                .plan_rename_with(&a.old, &a.new, a.include_subpages.unwrap_or(true))?;
            let plan_moves = p.file_moves.clone();
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
                warnings: {
                    let mut w = p.warnings;
                    // a page that moves into or out of an encrypted path is stored in the other form (EN-17)
                    let rules = o.graph.config().encryption.rules();
                    for (from, to) in &plan_moves {
                        match (rules.is_encrypted(from), rules.is_encrypted(to)) {
                            (false, true) => w.push(format!(
                                "`{from}` moves into an encrypted path: it will be stored encrypted on the server"
                            )),
                            (true, false) => w.push(format!(
                                "`{from}` moves out of an encrypted path: it will be stored as plain text on the server"
                            )),
                            _ => {}
                        }
                    }
                    w
                },
            })
        })
    }

    /// `rename_apply`
    pub fn rename_apply(&self, a: RenameArgs) -> ApiResult<PageView> {
        self.with_open(|o| {
            let plan =
                o.graph
                    .plan_rename_with(&a.old, &a.new, a.include_subpages.unwrap_or(true))?;
            if plan.file_moves.len() + plan.edits.len() > 1 {
                self.snapshot_before_change(o);
            }
            log::info!(
                "rename `{}` -> `{}`: {} file(s) moved, {} rewritten",
                plan.old,
                plan.new,
                plan.file_moves.len(),
                plan.edits.len()
            );
            if let Err(e) = o.graph.apply_rename(&plan) {
                log::error!(
                    "rename `{}` -> `{}` stopped half way: {e}",
                    plan.old,
                    plan.new
                );
                // remember it: the user can revert what was already written
                *self
                    .last_rename
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(plan);
                return Err(e.into());
            }
            *self
                .last_rename
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
            // a favourite follows the page, and the pages below it when they moved with it (NS-26)
            Self::rename_favorites(o, &plan.old, &plan.new, a.include_subpages.unwrap_or(true));
            Self::rename_hidden(o, &plan.old, &plan.new, a.include_subpages.unwrap_or(true));
            // history of the old name can't be undone safely
            o.undo.retain(|e| e.page != plan.old);
            o.redo.retain(|e| e.page != plan.old);
            let n = Self::parse_name(&plan.new)?;
            (self.emit)(
                "graph-changed",
                json!({ "pages": [plan.old.clone(), plan.new.clone()], "paths": [], "own": true }),
            );
            Ok(Self::build_view(o, &n)?.0)
        })
    }

    /// `delete_pages`: moves a page (and optionally everything below it) to the trash. Links to it stay: the page
    /// becomes an empty page again. `restore_pages` brings the files back.
    pub fn delete_pages(&self, a: DeletePagesArgs) -> ApiResult<DeletePagesResult> {
        self.with_open(|o| {
            let info = o
                .graph
                .with_index(|i| i.page_info(&a.page))?
                .ok_or_else(|| ApiError::new("not_found", format!("no page `{}`", a.page)))?;
            let mut pages: Vec<(String, String)> = Vec::new();
            if let Some(f) = info.file_path.clone() {
                pages.push((info.name.clone(), f));
            }
            if a.include_subpages {
                for s in o.graph.with_index(|i| i.subtree_pages(&info.name))? {
                    if let Some(f) = s.file_path {
                        pages.push((s.name, f));
                    }
                }
            }
            if pages.is_empty() {
                return Err(ApiError::new(
                    "invalid",
                    format!("`{}` has no file: there is nothing to delete", info.name),
                ));
            }
            self.delete_found(o, pages)
        })
    }

    /// Moves the files of `pages` (`(name, file)`) to the trash and cleans up after them: favorites, the undo
    /// history, an event. The part of `delete_pages` that `transfer_apply` shares (MG-2).
    fn delete_found(
        &self,
        o: &mut Open,
        pages: Vec<(String, String)>,
    ) -> ApiResult<DeletePagesResult> {
        let rels: Vec<String> = pages.iter().map(|p| p.1.clone()).collect();
        if rels.len() > 1 {
            self.snapshot_before_change(o);
        }
        let trash_id = o.graph.trash_pages(&rels)?;
        let names: Vec<String> = pages.into_iter().map(|p| p.0).collect();
        log::info!(
            "deleted {} page(s) to the trash ({trash_id}): {}",
            names.len(),
            names.join(", ")
        );
        // a deleted page is no favourite any more (NS-26)
        Self::remove_favorites(o, &names);
        Self::remove_hidden(o, &names);
        // the history of a deleted page can't be undone any more
        o.undo.retain(|e| !names.contains(&e.page));
        o.redo.retain(|e| !names.contains(&e.page));
        let mut linked_from: Vec<String> = Vec::new();
        for n in &names {
            for l in o.graph.with_index(|i| i.backlinks(n))? {
                if !names.contains(&l.source_page) && !linked_from.contains(&l.source_page) {
                    linked_from.push(l.source_page);
                }
            }
        }
        (self.emit)(
            "graph-changed",
            json!({ "pages": names.clone(), "paths": rels, "own": true }),
        );
        Ok(DeletePagesResult {
            trash_id,
            deleted: names,
            linked_from,
        })
    }

    /// `restore_pages`: puts deleted pages back (what `delete_pages` returned as `trash_id`).
    pub fn restore_pages(&self, a: RestorePagesArgs) -> ApiResult<StringList> {
        self.with_open(|o| {
            let restored = o.graph.restore_trash(&a.trash_id)?;
            log::info!(
                "restored {} file(s) from the trash ({})",
                restored.len(),
                a.trash_id
            );
            let pages: Vec<String> = restored
                .iter()
                .filter_map(|r| o.graph.with_index(|i| i.page_for_file(r)).ok().flatten())
                .collect();
            (self.emit)(
                "graph-changed",
                json!({ "pages": pages, "paths": restored.clone(), "own": true }),
            );
            Ok(StringList { items: pages })
        })
    }

    /// `archived_pages`: the hidden names: pages with `archived: true` and the `hidden` list (the pages below them are
    /// hidden too; the UI applies that).
    pub fn archived_pages(&self, _a: NoArgs) -> ApiResult<StringList> {
        self.with_open(|o| {
            Ok(StringList {
                items: Self::hidden_names(o)?,
            })
        })
    }

    /// `rename_revert`: undoes a rename that stopped half way (files rewritten before the conflict get their old
    /// text back, moved files return). Returns the restored paths.
    pub fn rename_revert(&self, _a: NoArgs) -> ApiResult<StringList> {
        self.with_open(|o| {
            let plan = self
                .last_rename
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .take()
                .ok_or_else(|| {
                    ApiError::new("invalid", "there is no unfinished rename to revert")
                })?;
            let restored = o.graph.revert_rename(&plan)?;
            log::info!("rename `{}` -> `{}` reverted", plan.old, plan.new);
            o.undo.retain(|e| e.page != plan.old && e.page != plan.new);
            o.redo.retain(|e| e.page != plan.old && e.page != plan.new);
            (self.emit)(
                "graph-changed",
                json!({ "pages": [plan.old.clone(), plan.new.clone()], "paths": [], "own": false }),
            );
            Ok(StringList { items: restored })
        })
    }

    /// `new_block_id`: an unused block ID, for a block that is about to be referenced (`/followup`).
    pub fn new_block_id(&self, _a: NoArgs) -> ApiResult<StringValue> {
        self.with_open(|o| {
            let used = o.graph.with_index(|i| i.all_block_uids())?;
            let mut state = now_seed();
            let mut rng = move || {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                state
            };
            Ok(StringValue {
                value: notatnik_core::ids::generate_block_id(&used, &mut rng),
            })
        })
    }

    /// `task_chain`: the follow-up chain of a task (TK-15).
    pub fn task_chain(&self, a: ChainArgs) -> ApiResult<ChainView> {
        let path = a
            .path
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(".");
        self.with_open(|o| {
            let found = o
                .graph
                .with_index(|i| i.task_chain(&a.page, &path))?
                .ok_or_else(|| ApiError::new("not_found", "no such task"))?;
            let to_path =
                |p: &str| -> Vec<u32> { p.split('.').filter_map(|x| x.parse().ok()).collect() };
            let items = found
                .0
                .into_iter()
                .map(|t| ChainItem {
                    uid: t.uid,
                    missing: t.missing,
                    path: to_path(&t.path),
                    text: t.text.lines().next().unwrap_or("").to_string(),
                    page: t.page,
                    state: t.task_state,
                    scheduled: t.scheduled,
                    deadline: t.deadline,
                    depth: u32::try_from(t.depth).unwrap_or(0),
                })
                .collect();
            Ok(ChainView {
                items,
                position: u32::try_from(found.1).unwrap_or(0),
            })
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

    /// `log_client`: a line of the UI's own for the terminal log (an error nobody caught). The webview's console is
    /// not visible in the desktop app, so the UI sends what it would have printed there.
    pub fn log_client(&self, a: LogClientArgs) -> ApiResult<NoArgs> {
        let level = match a.level.as_str() {
            "error" => log::Level::Error,
            "warn" => log::Level::Warn,
            "debug" => log::Level::Debug,
            _ => log::Level::Info,
        };
        let message: String = a.message.chars().take(2000).collect();
        log::log!(target: "ui", level, "{message}");
        Ok(NoArgs {})
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

    /// The graph-relative file of a page, also for one that has no file yet.
    fn page_file(o: &Open, name: &PageName) -> ApiResult<String> {
        let info = o.graph.with_index(|i| i.page_info(name.as_str()))?;
        let canonical = info
            .as_ref()
            .and_then(|i| PageName::parse(&i.name).ok())
            .unwrap_or_else(|| name.clone());
        Ok(info
            .and_then(|i| i.file_path)
            .unwrap_or_else(|| page_to_path(&canonical)))
    }

    /// `asset_link`: `![](...)` for an existing asset, with the path written relative to `page`'s file.
    pub fn asset_link(&self, a: AssetLinkArgs) -> ApiResult<StringValue> {
        let name = Self::parse_name(&a.page)?;
        self.with_open(|o| {
            if !a.path.starts_with("assets/") || a.path.contains("..") {
                return Err(ApiError::new("invalid", "not a path below assets/"));
            }
            let page_rel = Self::page_file(o, &name)?;
            Ok(StringValue {
                value: format!(
                    "![]({})",
                    notatnik_graph::relative_asset_path(&page_rel, &a.path)
                ),
            })
        })
    }

    /// `save_asset`: stores a pasted or dropped image and returns the Markdown that links it from `page` (ST-8).
    pub fn save_asset(&self, a: SaveAssetArgs) -> ApiResult<SavedAsset> {
        use base64::Engine as _;
        let name = Self::parse_name(&a.page)?;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(a.data.trim())
            .map_err(|_| ApiError::new("invalid", "the image data is not valid base64"))?;
        let today = self.today();
        self.with_open(|o| {
            let page_rel = Self::page_file(o, &name)?;
            let path = o.graph.save_asset(&bytes, a.name.as_deref(), today)?;
            let markdown = format!(
                "![]({})",
                notatnik_graph::relative_asset_path(&page_rel, &path)
            );
            Ok(SavedAsset { path, markdown })
        })
    }

    /// `attach_file`: copies a file into the graph's `assets/` and returns the Markdown link to it from `page`.
    pub fn attach_file(&self, a: AttachFileArgs) -> ApiResult<SavedAsset> {
        use base64::Engine as _;
        let name = Self::parse_name(&a.page)?;
        let (bytes, file_name) = match (&a.path, &a.data) {
            (Some(path), _) => {
                let p = Path::new(path);
                let meta = std::fs::metadata(p)
                    .map_err(|e| ApiError::new("not_found", format!("{path}: {e}")))?;
                if !meta.is_file() {
                    return Err(ApiError::new("invalid", "that is not a file"));
                }
                if meta.len() > notatnik_graph::MAX_ASSET_BYTES as u64 {
                    return Err(ApiError::new(
                        "invalid",
                        "the file is larger than 25 MB; link to it instead of copying it",
                    ));
                }
                let bytes = std::fs::read(p)
                    .map_err(|e| ApiError::new("not_found", format!("{path}: {e}")))?;
                let n = p
                    .file_name()
                    .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
                (bytes, n)
            }
            (None, Some(data)) => (
                base64::engine::general_purpose::STANDARD
                    .decode(data.trim())
                    .map_err(|_| ApiError::new("invalid", "the file data is not valid base64"))?,
                a.name.clone().unwrap_or_default(),
            ),
            (None, None) => return Err(ApiError::new("invalid", "no file was given")),
        };
        let today = self.today();
        self.with_open(|o| {
            let page_rel = Self::page_file(o, &name)?;
            let path = o.graph.save_attachment(&bytes, &file_name, today)?;
            let label = if file_name.is_empty() {
                path.rsplit('/').next().unwrap_or("file").to_string()
            } else {
                file_name.clone()
            };
            let label: String = label
                .chars()
                .flat_map(|c| {
                    if matches!(c, '[' | ']' | '\\') {
                        vec!['\\', c]
                    } else {
                        vec![c]
                    }
                })
                .collect();
            let markdown = format!(
                "[{label}]({})",
                notatnik_graph::relative_asset_path(&page_rel, &path)
            );
            Ok(SavedAsset { path, markdown })
        })
    }

    /// `list_orphan_assets`: files below `assets/` that no page mentions.
    pub fn orphan_assets(&self, _: NoArgs) -> ApiResult<OrphanAssets> {
        self.with_open(|o| {
            let items = o
                .graph
                .orphan_assets()?
                .into_iter()
                .map(|(path, size)| OrphanAsset {
                    path,
                    size: size as f64,
                })
                .collect();
            Ok(OrphanAssets { items })
        })
    }

    /// `delete_orphan_assets`: moves orphans to the trash; returns the orphans that are left.
    pub fn delete_orphan_assets(&self, a: DeleteAssetsArgs) -> ApiResult<OrphanAssets> {
        self.with_open(|o| {
            o.graph
                .trash_orphan_assets(&a.paths)
                .map(|_| ())
                .map_err(Into::into)
        })?;
        self.orphan_assets(NoArgs {})
    }

    /// Reads a picture of the graph for the webview: anything below `assets/`, or an image file elsewhere in the
    /// graph (Nextcloud keeps a page's attachments in `.attachments.N/` beside it).
    pub fn read_asset(&self, rel: &str) -> ApiResult<Vec<u8>> {
        let image = {
            let ext = rel
                .rsplit_once('.')
                .map_or("", |(_, e)| e)
                .to_ascii_lowercase();
            matches!(
                ext.as_str(),
                "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "avif" | "bmp"
            )
        };
        let full = self.graph_file(rel, |clean| clean.starts_with("assets/") || image)?;
        std::fs::read(full).map_err(|_| ApiError::new("not_found", "asset not found"))
    }

    /// The file for a link as the page view gives it (graph-relative; a leading `../` is ignored), checked to be
    /// inside the graph and outside `.notatnik`.
    pub fn asset_file(&self, rel: &str) -> ApiResult<PathBuf> {
        self.graph_file(rel, |_| true)
    }

    fn graph_file(&self, rel: &str, allowed: impl Fn(&str) -> bool) -> ApiResult<PathBuf> {
        self.with_open(|o| {
            let clean = rel.trim_start_matches("../").trim_start_matches("./");
            let p = Path::new(clean);
            if p.is_absolute()
                || p.components()
                    .any(|c| matches!(c, std::path::Component::ParentDir))
                || clean.starts_with(".notatnik")
                || !allowed(clean)
            {
                return Err(ApiError::new(
                    "invalid",
                    "only files of this graph (below assets/, or pictures) can be opened",
                ));
            }
            let full = o.graph.root().join(p);
            if full.is_file() {
                Ok(full)
            } else {
                Err(ApiError::new("not_found", "asset not found"))
            }
        })
    }
}

// ---- queries, agenda, saved queries, property completion ---------------------------------------

use notatnik_core::query::GroupBy;
use notatnik_index::{QueryContext, QueryRow};

/// `properties:` of a query: hides every property of the block and of its sub-blocks that is not in `keep`. They stay
/// in the view (editing finds them by key) and are only marked hidden, which is what the property card respects.
fn limit_properties(b: &mut BlockView, keep: &[String]) {
    for p in &mut b.props {
        if !keep.iter().any(|k| k.eq_ignore_ascii_case(&p.key)) {
            p.hidden = true;
        }
    }
    for c in &mut b.children {
        limit_properties(c, keep);
    }
}

fn breadcrumbs_of(blocks: &[BlockView], path: &[u32]) -> Vec<String> {
    let mut out = Vec::new();
    let mut list = blocks;
    for i in path.iter().take(path.len().saturating_sub(1)) {
        match list.get(*i as usize) {
            Some(b) => {
                out.push(b.text.lines().next().unwrap_or("").to_string());
                list = &b.children;
            }
            None => break,
        }
    }
    out
}

impl AppState {
    fn context(&self, o: &Open) -> QueryContext {
        QueryContext {
            today: self.today(),
            week_start: o.graph.config().week_start(),
        }
    }

    /// Looks up the rendered block of a result row, caching page views for the call.
    fn row_block(
        o: &mut Open,
        cache: &mut HashMap<String, PageView>,
        r: &QueryRow,
        children: bool,
        properties: Option<&[String]>,
    ) -> ApiResult<(Option<BlockView>, Vec<String>)> {
        let Some(path) = r.path.as_deref() else {
            return Ok((None, Vec::new()));
        };
        if path == "p" {
            return Ok((None, Vec::new()));
        }
        if !cache.contains_key(&r.page) {
            let n = Self::parse_name(&r.page)?;
            cache.insert(r.page.clone(), Self::build_view(o, &n)?.0);
        }
        let p: Vec<u32> = path.split('.').filter_map(|x| x.parse().ok()).collect();
        let Some(view) = cache.get(&r.page) else {
            return Ok((None, Vec::new()));
        };
        let crumbs = breadcrumbs_of(&view.blocks, &p);
        let block = find_block(&view.blocks, &p).map(|b| {
            let mut b = b.clone();
            if !children {
                b.children = Vec::new();
            }
            if let Some(keep) = properties {
                limit_properties(&mut b, keep);
            }
            b
        });
        Ok((block, crumbs))
    }

    /// `run_query`
    pub fn run_query(&self, a: RunQueryArgs) -> ApiResult<QueryView> {
        let today = self.today().format("%Y-%m-%d").to_string();
        // `{{type}}` and `{{prop.<key>}}` need the page's properties (read only when the query uses them)
        let props = match a.page.as_deref() {
            Some(p) if a.text.contains("{{") => self
                .with_open(|o| {
                    let rel = Self::page_rel(o, p)?;
                    Ok(o.graph
                        .read_page(&rel)
                        .map(|f| templates::page_props(&f.text))
                        .unwrap_or_default())
                })
                .unwrap_or_default(),
            _ => Vec::new(),
        };
        let text = match templates::expand_query(&a.text, a.page.as_deref(), &today, &props) {
            Ok(t) => t,
            Err((line, column, message)) => {
                return Ok(QueryView {
                    errors: vec![QueryErrorView {
                        line: u32::try_from(line).unwrap_or(1),
                        column: u32::try_from(column).unwrap_or(1),
                        message,
                        suggestion: None,
                    }],
                    target: "block".into(),
                    view: a.view.clone().unwrap_or_else(|| "list".into()),
                    columns: Vec::new(),
                    group_by: None,
                    expanded: None,
                    rows: Vec::new(),
                });
            }
        };
        // what the placeholders became, for `show query` (only when there was something to replace)
        let expanded = (text != a.text).then(|| text.clone());
        let (query, errs) = notatnik_core::parse_query_partial(&text);
        let view_name = a
            .view
            .clone()
            .or(query.view.clone())
            .unwrap_or_else(|| "list".into());
        let target = if query.target == notatnik_core::query::QueryTarget::Page {
            "page"
        } else {
            "block"
        };
        let group_by = query.group.as_ref().map(|g| match g {
            GroupBy::Page => "page".to_string(),
            GroupBy::Task => "task".into(),
            GroupBy::Scheduled => "scheduled".into(),
            GroupBy::Deadline => "deadline".into(),
            GroupBy::Prop(k) => format!("prop.{k}"),
        });
        if !errs.is_empty() {
            return Ok(QueryView {
                errors: errs
                    .into_iter()
                    .map(|e| QueryErrorView {
                        line: e.line as u32,
                        column: e.column as u32,
                        message: e.message,
                        suggestion: e.suggestion,
                    })
                    .collect(),
                target: target.into(),
                view: view_name,
                columns: query.columns,
                group_by,
                expanded,
                rows: Vec::new(),
            });
        }
        self.with_open(|o| {
            let ctx = self.context(o);
            let res = o.graph.with_index(|i| i.run_query(&query, &ctx));
            let res = match res {
                Ok(r) => r,
                Err(e) => {
                    return Ok(QueryView {
                        errors: vec![QueryErrorView {
                            line: 1,
                            column: 1,
                            message: e.to_string(),
                            suggestion: None,
                        }],
                        target: target.into(),
                        view: view_name,
                        columns: query.columns.clone(),
                        group_by,
                        expanded: expanded.clone(),
                        rows: Vec::new(),
                    });
                }
            };
            let mut cache: HashMap<String, PageView> = HashMap::new();
            let hidden: HashSet<String> =
                o.graph.config().properties.hidden.iter().cloned().collect();
            let mut keys: BTreeMap<String, usize> = BTreeMap::new();
            let mut rows = Vec::with_capacity(res.rows.len());
            for r in res.rows {
                let (block, breadcrumbs) = Self::row_block(
                    o,
                    &mut cache,
                    &r,
                    query.children,
                    query.properties.as_deref(),
                )?;
                for k in r.props.keys().filter(|k| !hidden.contains(*k)) {
                    *keys.entry(k.clone()).or_default() += 1;
                }
                rows.push(ResultRow {
                    id: r.id,
                    page: r.page,
                    group: r.group,
                    group_link: r.group_link,
                    block,
                    breadcrumbs,
                    props: r.props,
                    scheduled: r.scheduled,
                    deadline: r.deadline,
                    task_state: r.task_state,
                });
            }
            let columns = if query.columns.is_empty() {
                let mut c = vec![if target == "page" {
                    "page".to_string()
                } else {
                    "text".to_string()
                }];
                c.extend(keys.keys().map(|k| format!("prop.{k}")));
                c
            } else {
                query.columns.clone()
            };
            Ok(QueryView {
                errors: Vec::new(),
                target: target.into(),
                view: view_name,
                columns,
                group_by,
                expanded,
                rows,
            })
        })
    }

    /// `agenda`
    pub fn agenda(&self, a: AgendaArgs) -> ApiResult<AgendaView> {
        let mut text = String::from("task: todo, later, doing, now, waiting\n");
        let name = |n: &str| format!("[[{}]]", n.trim_start_matches('#'));
        if let Some(ns) = &a.ns {
            text.push_str(&format!("ns: {}\n", name(ns)));
        }
        if let Some(tag) = &a.tag {
            text.push_str(&format!("tag: {}\n", name(tag)));
        }
        if let Some(p) = &a.priority {
            let labels = self.with_open(|o| Ok(o.graph.config().priority_labels()))?;
            if !labels.iter().any(|l| l.eq_ignore_ascii_case(p)) {
                return Err(ApiError::new(
                    "invalid",
                    format!(
                        "unknown priority `{p}` (the labels are {})",
                        labels.join(", ")
                    ),
                ));
            }
            text.push_str(&format!("priority: {p}\n"));
        }
        let (query, errs) = notatnik_core::parse_query_partial(&text);
        if let Some(e) = errs.first() {
            return Err(ApiError::new("invalid", e.to_string()));
        }
        let today = self.today();
        let today_s = today.format("%Y-%m-%d").to_string();
        let horizon = today
            .checked_add_days(Days::new(7))
            .unwrap_or(today)
            .format("%Y-%m-%d")
            .to_string();
        self.with_open(|o| {
            let ctx = self.context(o);
            let res = o.graph.with_index(|i| i.run_query(&query, &ctx))?;
            let mut cache: HashMap<String, PageView> = HashMap::new();
            let mut view = AgendaView {
                today: today_s.clone(),
                overdue: Vec::new(),
                today_items: Vec::new(),
                upcoming: Vec::new(),
                later: Vec::new(),
                undated: Vec::new(),
            };
            let mut days: BTreeMap<String, Vec<AgendaItem>> = BTreeMap::new();
            // tasks of archived pages (and pages below them) are not on the agenda
            let archived: Vec<String> = Self::hidden_names(o)?
                .into_iter()
                .map(|n| n.to_lowercase())
                .collect();
            for r in res.rows {
                let lower = r.page.to_lowercase();
                if archived
                    .iter()
                    .any(|a| lower == *a || lower.starts_with(&format!("{a}/")))
                {
                    continue;
                }
                // the earlier of scheduled and deadline, with its start time (a task without one sorts last in
                // its day)
                let (date, time) = [
                    (&r.scheduled, &r.scheduled_time),
                    (&r.deadline, &r.deadline_time),
                ]
                .into_iter()
                .filter_map(|(d, t)| Some((d.clone()?, t.clone())))
                .min_by(|a, b| (&a.0, a.1.is_none(), &a.1).cmp(&(&b.0, b.1.is_none(), &b.1)))
                .map_or((None, None), |(d, t)| (Some(d), t));
                let (block, crumbs) = Self::row_block(
                    o,
                    &mut cache,
                    &r,
                    query.children,
                    query.properties.as_deref(),
                )?;
                let Some(block) = block else { continue };
                let item = AgendaItem {
                    page: r.page,
                    block,
                    breadcrumbs: crumbs,
                    date: date.clone(),
                    time,
                };
                match date {
                    None => view.undated.push(item),
                    Some(d) if d < today_s => view.overdue.push(item),
                    Some(d) if d == today_s => view.today_items.push(item),
                    Some(d) if d <= horizon => days.entry(d).or_default().push(item),
                    Some(_) => view.later.push(item),
                }
            }
            // within a day: by start time, tasks without one last (stable: the query's order otherwise)
            let by_time = |x: &AgendaItem| (x.time.is_none(), x.time.clone());
            view.today_items.sort_by_key(by_time);
            for items in days.values_mut() {
                items.sort_by_key(by_time);
            }
            view.overdue.sort_by(|x, y| x.date.cmp(&y.date));
            view.later.sort_by(|x, y| x.date.cmp(&y.date));
            view.upcoming = days
                .into_iter()
                .map(|(date, items)| AgendaDay { date, items })
                .collect();
            Ok(view)
        })
    }

    fn saved(o: &Open) -> Vec<SavedQueryView> {
        o.graph
            .config()
            .saved_queries
            .into_iter()
            .map(|q| SavedQueryView {
                name: q.name,
                query: q.query,
            })
            .collect()
    }

    /// `list_saved_queries`
    pub fn list_saved_queries(&self, _a: NoArgs) -> ApiResult<Vec<SavedQueryView>> {
        self.with_open(|o| Ok(Self::saved(o)))
    }

    /// `save_query`: adds or replaces a pinned query (stored in `config.toml`, so it syncs).
    pub fn save_query(&self, a: SavedQueryView) -> ApiResult<Vec<SavedQueryView>> {
        if a.name.trim().is_empty() {
            return Err(ApiError::new("invalid", "a saved query needs a name"));
        }
        let (_, errs) = notatnik_core::parse_query_partial(&a.query);
        if let Some(e) = errs.first() {
            return Err(ApiError::new("invalid", e.to_string()));
        }
        self.with_open(|o| {
            o.graph.update_config(|c| {
                let new = notatnik_graph::SavedQuery {
                    name: a.name.clone(),
                    query: a.query.clone(),
                };
                // a name that exists is replaced where it is, a new one goes to the end
                match c.saved_queries.iter_mut().find(|q| q.name == a.name) {
                    Some(q) => *q = new,
                    None => c.saved_queries.push(new),
                }
            })?;
            Ok(Self::saved(o))
        })
    }

    /// `edit_saved_query`: changes the name and the text of a saved query where it stands in the list.
    pub fn edit_saved_query(&self, a: EditSavedQueryArgs) -> ApiResult<Vec<SavedQueryView>> {
        if a.name.trim().is_empty() {
            return Err(ApiError::new("invalid", "a saved query needs a name"));
        }
        let (_, errs) = notatnik_core::parse_query_partial(&a.query);
        if let Some(e) = errs.first() {
            return Err(ApiError::new("invalid", e.to_string()));
        }
        self.with_open(|o| {
            let cfg = o.graph.config();
            if !cfg.saved_queries.iter().any(|q| q.name == a.old) {
                return Err(ApiError::new(
                    "not_found",
                    format!("no saved query `{}`", a.old),
                ));
            }
            if a.name != a.old && cfg.saved_queries.iter().any(|q| q.name == a.name) {
                return Err(ApiError::new(
                    "exists",
                    format!("a saved query called `{}` exists", a.name),
                ));
            }
            o.graph.update_config(|c| {
                if let Some(q) = c.saved_queries.iter_mut().find(|q| q.name == a.old) {
                    q.name = a.name.clone();
                    q.query = a.query.clone();
                }
            })?;
            Ok(Self::saved(o))
        })
    }

    /// `delete_saved_query`
    pub fn delete_saved_query(&self, a: NameArgs) -> ApiResult<Vec<SavedQueryView>> {
        self.with_open(|o| {
            o.graph
                .update_config(|c| c.saved_queries.retain(|q| q.name != a.name))?;
            Ok(Self::saved(o))
        })
    }

    /// `property_keys`
    pub fn property_keys(&self, _a: NoArgs) -> ApiResult<StringList> {
        self.with_open(|o| {
            Ok(StringList {
                items: o.graph.with_index(|i| i.property_keys(60))?,
            })
        })
    }

    /// `property_values`
    pub fn property_values(&self, a: PropKeyArgs) -> ApiResult<StringList> {
        self.with_open(|o| {
            Ok(StringList {
                items: o.graph.with_index(|i| i.property_values(&a.key, 40))?,
            })
        })
    }
}

impl AppState {
    fn settings_view(&self, o: &Open) -> ApiResult<SettingsView> {
        let dev = notatnik_graph::DeviceSettings::load(&self.dirs)?;
        let cfg = o.graph.config();
        Ok(SettingsView {
            theme: dev.ui.theme,
            date_locale: dev.ui.date_locale,
            link_brackets: dev.ui.link_brackets,
            paste_titles: dev.ui.paste_titles,
            icon_style: dev.ui.icon_style,
            task_details: dev.ui.task_details,
            block_properties: dev.ui.block_properties,
            date_format: dev.ui.date_format,
            date_pattern: dev.ui.date_pattern,
            backup_dir: dev.ui.backup_dir,
            backup_auto: dev.ui.backup_auto,
            backup_compression: dev.ui.backup_compression,
            backup_mirror_dir: dev.ui.backup_mirror_dir,
            backup_encrypt: dev.ui.backup_encrypt,
            templates_auto: dev.ui.templates_auto,
            open_last_graph: dev.ui.open_last_graph,
            journal_template: cfg
                .graph
                .journal_template
                .as_deref()
                .and_then(templates::template_name)
                .unwrap_or_default(),
            namespace_templates: cfg
                .namespaces
                .iter()
                .filter_map(|(ns, sec)| {
                    Some(NamespaceTemplateView {
                        namespace: ns.clone(),
                        template: templates::template_name(sec.template.as_deref()?)?,
                    })
                })
                .collect(),
            priorities: cfg.priority_labels(),
            week_start: cfg.graph.week_start.clone(),
            completed_log_limit: cfg.tasks.completed_log_limit,
            page_icons: cfg
                .page_icons
                .iter()
                .map(|r| PageIconRuleView {
                    key: r.key.clone(),
                    value: r.value.clone(),
                    icon: r.icon.clone(),
                })
                .collect(),
            graph_root: o.graph.root().to_string_lossy().into_owned(),
        })
    }

    /// `get_settings`
    pub fn get_settings(&self, _a: NoArgs) -> ApiResult<SettingsView> {
        self.with_open(|o| self.settings_view(o))
    }

    /// `set_settings`: device settings only touch `settings.toml`, graph settings only `config.toml`.
    pub fn set_settings(&self, a: SetSettingsArgs) -> ApiResult<SettingsView> {
        if let Some(t) = &a.theme {
            if !matches!(t.as_str(), "system" | "light" | "dark") {
                return Err(ApiError::new(
                    "invalid",
                    "theme must be system, light or dark",
                ));
            }
        }
        if let Some(rules) = &a.page_icons {
            for r in rules {
                let (k, v, i) = (r.key.trim(), r.value.trim(), r.icon.trim());
                if k.is_empty() || v.is_empty() || i.is_empty() {
                    return Err(ApiError::new(
                        "invalid",
                        "a page icon needs a property, a value and an icon",
                    ));
                }
                if k.chars().count() > 60 || v.chars().count() > 120 || i.chars().count() > 16 {
                    return Err(ApiError::new(
                        "invalid",
                        "page icon: property or value too long, or more than a few characters of icon",
                    ));
                }
                if k.chars().any(char::is_control) || v.chars().any(char::is_control) {
                    return Err(ApiError::new("invalid", "page icon: no control characters"));
                }
            }
        }
        if let Some(m) = &a.icon_style {
            if !matches!(m.as_str(), "color" | "muted" | "mono") {
                return Err(ApiError::new(
                    "invalid",
                    "icon_style must be color, muted or mono",
                ));
            }
        }
        for (name, v) in [
            ("task_details", &a.task_details),
            ("block_properties", &a.block_properties),
        ] {
            if let Some(m) = v {
                if !matches!(m.as_str(), "full" | "compact") {
                    return Err(ApiError::new(
                        "invalid",
                        format!("{name} must be full or compact"),
                    ));
                }
            }
        }
        if let Some(f) = &a.date_format {
            if !notatnik_graph::DATE_FORMATS.contains(&f.as_str()) {
                return Err(ApiError::new(
                    "invalid",
                    "date_format must be full, long, short, iso or custom",
                ));
            }
        }
        if let Some(p) = &a.date_pattern {
            if p.chars().count() > 80 || p.chars().any(char::is_control) {
                return Err(ApiError::new(
                    "invalid",
                    "date_pattern must be at most 80 characters, without control characters",
                ));
            }
        }
        if let Some(d) = &a.backup_dir {
            let d = d.trim();
            if d.chars().count() > 500 || d.chars().any(char::is_control) {
                return Err(ApiError::new("invalid", "backup_dir: not a folder path"));
            }
            if !d.is_empty() && !Path::new(d).is_absolute() {
                return Err(ApiError::new(
                    "invalid",
                    "backup_dir must be an absolute path (or empty for the default)",
                ));
            }
        }
        if let Some(d) = &a.backup_mirror_dir {
            let d = d.trim();
            if d.chars().count() > 500 || d.chars().any(char::is_control) {
                return Err(ApiError::new(
                    "invalid",
                    "backup_mirror_dir: not a folder path",
                ));
            }
            if !d.is_empty() && !Path::new(d).is_absolute() {
                return Err(ApiError::new(
                    "invalid",
                    "backup_mirror_dir must be an absolute path (or empty for none)",
                ));
            }
        }
        if let Some(c) = &a.backup_compression {
            if !matches!(c.as_str(), "compressed" | "none") {
                return Err(ApiError::new(
                    "invalid",
                    "backup_compression must be compressed or none",
                ));
            }
        }
        if let Some(l) = &a.date_locale {
            // a BCP 47 tag, e.g. `pl-PL`; the UI only offers known ones, this guards the settings file
            let ok = !l.is_empty()
                && l.len() <= 20
                && l.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
            if !ok {
                return Err(ApiError::new(
                    "invalid",
                    "date_locale must be a locale tag like pl-PL",
                ));
            }
        }
        if let Some(w) = &a.week_start {
            if !matches!(w.as_str(), "monday" | "sunday") {
                return Err(ApiError::new(
                    "invalid",
                    "week_start must be monday or sunday",
                ));
            }
        }
        self.with_open(|o| {
            if a.theme.is_some()
                || a.date_locale.is_some()
                || a.link_brackets.is_some()
                || a.paste_titles.is_some()
                || a.icon_style.is_some()
                || a.task_details.is_some()
                || a.block_properties.is_some()
                || a.date_format.is_some()
                || a.date_pattern.is_some()
                || a.backup_dir.is_some()
                || a.backup_auto.is_some()
                || a.backup_compression.is_some()
                || a.backup_mirror_dir.is_some()
                || a.backup_encrypt.is_some()
                || a.templates_auto.is_some()
                || a.open_last_graph.is_some()
            {
                for d in [a.backup_dir.as_deref(), a.backup_mirror_dir.as_deref()]
                    .into_iter()
                    .flatten()
                    .map(str::trim)
                {
                    if !d.is_empty() && Path::new(d).starts_with(o.graph.root()) {
                        return Err(ApiError::new(
                            "invalid",
                            "the backup folder must be outside the graph folder",
                        ));
                    }
                }
                let mut dev = notatnik_graph::DeviceSettings::load(&self.dirs)?;
                if let Some(t) = a.theme {
                    dev.ui.theme = t;
                }
                if let Some(l) = a.date_locale {
                    dev.ui.date_locale = l;
                }
                if let Some(f) = a.date_format {
                    dev.ui.date_format = f;
                }
                if let Some(p) = a.date_pattern {
                    dev.ui.date_pattern = p;
                }
                if let Some(b) = a.link_brackets {
                    dev.ui.link_brackets = b;
                }
                if let Some(b) = a.paste_titles {
                    dev.ui.paste_titles = b;
                }
                if let Some(m) = a.icon_style {
                    dev.ui.icon_style = m;
                }
                if let Some(m) = a.task_details {
                    dev.ui.task_details = m;
                }
                if let Some(m) = a.block_properties {
                    dev.ui.block_properties = m;
                }
                if let Some(d) = a.backup_dir {
                    dev.ui.backup_dir = d.trim().to_string();
                }
                if let Some(b) = a.backup_auto {
                    dev.ui.backup_auto = b;
                }
                if let Some(c) = a.backup_compression {
                    dev.ui.backup_compression = c;
                }
                if let Some(d) = a.backup_mirror_dir {
                    dev.ui.backup_mirror_dir = d.trim().to_string();
                }
                if let Some(on) = a.backup_encrypt {
                    // turning it on needs a confirmed vault; turning it off always works
                    if on && !matches!(load_vault(&vault_path(&self.dirs)), Ok(Some(_))) {
                        return Err(ApiError::new(
                            "invalid",
                            "set up backup encryption first (Settings → Backup → Encryption)",
                        ));
                    }
                    dev.ui.backup_encrypt = on;
                }
                if mirror_dir(&dev).is_some_and(|m| m == primary_dir(&self.dirs, &dev)) {
                    return Err(ApiError::new(
                        "invalid",
                        "the second backup folder must differ from the first",
                    ));
                }
                if let Some(b) = a.templates_auto {
                    dev.ui.templates_auto = b;
                }
                if let Some(b) = a.open_last_graph {
                    dev.ui.open_last_graph = b;
                }
                dev.save(&self.dirs)?;
            }
            if let Some(p) = &a.priorities {
                notatnik_graph::validate_priorities(p).map_err(|m| ApiError::new("invalid", m))?;
            }
            if a.week_start.is_some()
                || a.completed_log_limit.is_some()
                || a.page_icons.is_some()
                || a.priorities.is_some()
                || a.journal_template.is_some()
                || a.namespace_templates.is_some()
            {
                o.graph.update_config(|c| {
                    if let Some(j) = &a.journal_template {
                        c.graph.journal_template =
                            templates::template_name(j).map(|n| format!("templates/{n}.md"));
                    }
                    if let Some(list) = &a.namespace_templates {
                        c.namespaces = list
                            .iter()
                            .filter_map(|r| {
                                let ns = r.namespace.trim().trim_matches('/').to_string();
                                let t = templates::template_name(&r.template)?;
                                (!ns.is_empty()).then(|| {
                                    (
                                        ns,
                                        notatnik_graph::NamespaceSection {
                                            template: Some(format!("templates/{t}.md")),
                                        },
                                    )
                                })
                            })
                            .collect();
                    }
                    if let Some(rules) = &a.page_icons {
                        c.page_icons = rules
                            .iter()
                            .map(|r| notatnik_graph::PageIconRule {
                                key: r.key.trim().to_string(),
                                value: r.value.trim().to_string(),
                                icon: r.icon.trim().to_string(),
                            })
                            .collect();
                    }
                    if let Some(w) = &a.week_start {
                        c.graph.week_start = w.clone();
                    }
                    if let Some(n) = a.completed_log_limit {
                        c.tasks.completed_log_limit = n;
                    }
                    if let Some(p) = &a.priorities {
                        c.tasks.priorities = p.iter().map(|l| l.trim().to_string()).collect();
                    }
                })?;
            }
            self.settings_view(o)
        })
    }

    // ---- backups (BK-1..BK-5) ---------------------------------------------------------------

    fn default_backup_dir(&self) -> PathBuf {
        self.dirs.data.join("backups")
    }

    /// The first snapshot folder: the device setting, or the default.
    fn backup_dir(&self) -> PathBuf {
        let dev = notatnik_graph::DeviceSettings::load(&self.dirs).unwrap_or_default();
        primary_dir(&self.dirs, &dev)
    }

    fn backup_list(
        &self,
        graph: &Graph,
        error: Option<String>,
        mirror_error: Option<String>,
    ) -> BackupList {
        let dev = notatnik_graph::DeviceSettings::load(&self.dirs).unwrap_or_default();
        let dir = primary_dir(&self.dirs, &dev);
        let mirror = mirror_dir(&dev).map(|m| graph.backups(&m));
        let backups = graph
            .backups(&dir)
            .into_iter()
            .map(|b| BackupView {
                encrypted: b.encrypted,
                in_mirror: mirror
                    .as_ref()
                    .is_some_and(|m| m.iter().any(|x| x.name == b.name && x.bytes == b.bytes)),
                name: b.name,
                created: b.created.format("%Y-%m-%d %H:%M:%S").to_string(),
                bytes: b.bytes as f64,
                before: b.before,
            })
            .collect();
        BackupList {
            dir: dir.to_string_lossy().into_owned(),
            default_dir: self.default_backup_dir().to_string_lossy().into_owned(),
            backups,
            error,
            mirror_dir: dev.ui.backup_mirror_dir,
            mirror_error,
        }
    }

    /// Starts the automatic snapshots: first one a few seconds after the graph opens, then every hour a check
    /// that takes one when the newest is a day old and the files changed. The settings are read at every check.
    fn start_backups(&self, graph: &Graph) -> BackupTask {
        let stop = Arc::new(AtomicBool::new(false));
        let error = Arc::new(Mutex::new(None));
        let mirror_error = Arc::new(Mutex::new(None));
        let (g, dirs) = (graph.clone(), self.dirs.clone());
        let (stop2, error2, mirror2) = (stop.clone(), error.clone(), mirror_error.clone());
        let spawned = std::thread::Builder::new()
            .name("backup".into())
            .spawn(move || {
                let wait = |secs: u64| {
                    for _ in 0..secs {
                        if stop2.load(Ordering::Relaxed) {
                            return false;
                        }
                        std::thread::sleep(Duration::from_secs(1));
                    }
                    !stop2.load(Ordering::Relaxed)
                };
                let mut next = 10;
                while wait(next) {
                    next = 3600;
                    let dev = notatnik_graph::DeviceSettings::load(&dirs).unwrap_or_default();
                    if !dev.ui.backup_auto {
                        continue;
                    }
                    let (made, mirror) =
                        run_backup(&dirs, &g, false, Some(chrono::Duration::hours(24)), None);
                    put(&error2, made.err().map(|e| e.to_string()));
                    put(&mirror2, mirror);
                }
            });
        // no thread, no automatic snapshots; "Back up now" still works
        drop(spawned);
        BackupTask {
            stop,
            error,
            mirror_error,
        }
    }

    /// A snapshot when the app quits (BK-3): taken when automatic backups are on, something changed and the newest
    /// snapshot is at least an hour old; the second folder is brought up to date too. The caller bounds the wait.
    pub fn backup_on_quit(&self) {
        self.backup_on_quit_own();
        for s in self.other_states() {
            s.backup_on_quit_own();
        }
    }

    /// The snapshot of the graph of this state only.
    pub(super) fn backup_on_quit_own(&self) {
        let dev = notatnik_graph::DeviceSettings::load(&self.dirs).unwrap_or_default();
        if !dev.ui.backup_auto {
            return;
        }
        let g = self.lock();
        let Some(o) = g.as_ref() else { return };
        if let Some(b) = &o.backup {
            b.stop.store(true, Ordering::Relaxed);
        }
        let _ = run_backup(
            &self.dirs,
            &o.graph,
            false,
            Some(chrono::Duration::hours(1)),
            self.unlocked_keys(),
        );
    }

    /// A restore point before a rename or delete that touches several files (BK-6). Skipped when automatic backups
    /// are off, and never blocks the operation: a failure only means there is no restore point.
    fn snapshot_before_change(&self, o: &Open) {
        let dev = notatnik_graph::DeviceSettings::load(&self.dirs).unwrap_or_default();
        if !dev.ui.backup_auto {
            return;
        }
        // without a usable vault nothing is written, never a plain snapshot in its place
        let Ok(opts) = backup_options(&self.dirs, &dev, &o.graph.id(), self.unlocked_keys(), true)
        else {
            return;
        };
        match o.graph.backup_before_change(
            &primary_dir(&self.dirs, &dev),
            chrono::Local::now().naive_local(),
            &opts,
        ) {
            Ok(Some(f)) => log::info!("restore point {} written before a bulk change", f.name),
            Ok(None) => {}
            Err(e) => log::warn!("no restore point before a bulk change: {e}"),
        }
    }

    // ---- page history (VH-1..VH-3) ------------------------------------------------------------

    /// The file of a page: the indexed one, or where it would be created.
    fn page_rel(o: &Open, page: &str) -> ApiResult<String> {
        let name = Self::parse_name(page)?;
        let info = o.graph.with_index(|i| i.page_info(name.as_str()))?;
        Ok(info
            .and_then(|i| i.file_path)
            .unwrap_or_else(|| page_to_path(&name)))
    }

    /// `page_versions`: the recorded states of the page's file, newest first.
    pub fn page_versions(&self, a: PageArgs) -> ApiResult<Vec<PageVersionView>> {
        self.with_open(|o| {
            let rel = Self::page_rel(o, &a.page)?;
            Ok(o.graph
                .page_versions(&rel)?
                .into_iter()
                .map(|v| PageVersionView {
                    id: v.id,
                    at_ms: v.at_ms as f64,
                    bytes: v.bytes as f64,
                })
                .collect())
        })
    }

    /// `page_version_text`
    pub fn page_version_text(&self, a: PageVersionArgs) -> ApiResult<StringValue> {
        self.with_open(|o| {
            let rel = Self::page_rel(o, &a.page)?;
            // `current` is the file as it is now, for the comparison
            let value = if a.id == "current" {
                o.graph.read_page(&rel).map(|p| p.text).unwrap_or_default()
            } else {
                o.graph.read_page_version(&rel, &a.id)?
            };
            Ok(StringValue { value })
        })
    }

    /// `restore_page_version`: the version becomes the file; the text it replaces is recorded as a version.
    pub fn restore_page_version(&self, a: PageVersionArgs) -> ApiResult<StringList> {
        self.with_open(|o| {
            let rel = Self::page_rel(o, &a.page)?;
            o.graph.restore_page_version(&rel, &a.id)?;
            log::info!("restored an earlier version of {rel}");
            o.undo.retain(|e| e.rel != rel);
            o.redo.retain(|e| e.rel != rel);
            (self.emit)(
                "graph-changed",
                json!({ "pages": [a.page.clone()], "paths": [rel], "own": true }),
            );
            Ok(StringList {
                items: vec![a.page],
            })
        })
    }

    // ---- backup encryption (EN-2, EN-5, EN-6) -------------------------------------------------

    /// The unlocked keys, or None when locked. Using them counts as use; they lock after 15 idle minutes.
    fn unlocked_keys(&self) -> Option<Arc<notatnik_crypt::Keys>> {
        let mut g = self
            .backup_keys
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match g.as_mut() {
            Some(u) if u.last_use.elapsed() < BACKUP_LOCK_AFTER => {
                u.last_use = std::time::Instant::now();
                Some(u.keys.clone())
            }
            Some(_) => {
                *g = None;
                None
            }
            None => None,
        }
    }

    fn set_unlocked(&self, keys: Option<notatnik_crypt::Keys>) {
        *self
            .backup_keys
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = keys.map(|k| Unlocked {
            keys: Arc::new(k),
            last_use: std::time::Instant::now(),
        });
    }

    /// Options for reading snapshots (compare, restore, device settings): the keys if unlocked, no sealing.
    fn read_options(&self, graph_id: &str) -> ApiResult<notatnik_graph::BackupOptions> {
        let dev = notatnik_graph::DeviceSettings::load(&self.dirs).unwrap_or_default();
        let mut opts = backup_options(&self.dirs, &dev, graph_id, self.unlocked_keys(), false)
            .map_err(|m| ApiError::new("invalid", m))?;
        // the files a snapshot holds sealed for the graph (EN-23) open with the graph's key
        opts.graph_keys = self.graph_keys_now();
        Ok(opts)
    }

    fn encryption_status(&self) -> BackupEncryption {
        let dev = notatnik_graph::DeviceSettings::load(&self.dirs).unwrap_or_default();
        let vault = load_vault(&vault_path(&self.dirs)).ok().flatten();
        // peek without counting as use
        let unlocked = self
            .backup_keys
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
            .is_some_and(|u| u.last_use.elapsed() < BACKUP_LOCK_AFTER);
        let plain = self.lock().as_ref().map_or(0, |o| {
            let mut dirs = vec![primary_dir(&self.dirs, &dev)];
            dirs.extend(mirror_dir(&dev));
            dirs.iter()
                .map(|d| o.graph.backups(d).iter().filter(|b| !b.encrypted).count())
                .sum()
        });
        BackupEncryption {
            enabled: dev.ui.backup_encrypt,
            configured: vault.is_some(),
            pending: pending_vault_path(&self.dirs).exists(),
            unlocked,
            key_id: vault.map(|v| v.key_id).unwrap_or_default(),
            plain_snapshots: plain as u32,
        }
    }

    fn check_new_password(p: &str) -> ApiResult<()> {
        if p.chars().count() < 8 || p.chars().count() > 200 {
            return Err(ApiError::new(
                "invalid",
                "the password needs 8 to 200 characters",
            ));
        }
        Ok(())
    }

    fn configured_vault(&self) -> ApiResult<notatnik_crypt::Vault> {
        load_vault(&vault_path(&self.dirs))
            .map_err(|m| ApiError::new("invalid", m))?
            .ok_or_else(|| ApiError::new("invalid", "backup encryption is not set up"))
    }

    /// `backup_encryption_status`
    pub fn backup_encryption_status(&self, _a: NoArgs) -> ApiResult<BackupEncryption> {
        Ok(self.encryption_status())
    }

    /// `backup_encryption_setup`: makes a vault and returns its recovery key, to be shown once. Encryption turns on
    /// only with `backup_encryption_confirm`, after the user said the key is saved.
    pub fn backup_encryption_setup(&self, a: BackupSetupArgs) -> ApiResult<RecoveryKeyResult> {
        Self::check_new_password(&a.password)?;
        if vault_path(&self.dirs).exists() && a.reset != Some(true) {
            return Err(ApiError::new(
                "invalid",
                "backup encryption is already set up: change the password or make a new recovery key instead",
            ));
        }
        let (vault, keys, recovery) =
            notatnik_crypt::Vault::create(&a.password, notatnik_crypt::KdfParams::default())
                .map_err(|e| ApiError::new("internal", e.to_string()))?;
        save_vault(&pending_vault_path(&self.dirs), &vault).map_err(notatnik_graph::Error::from)?;
        self.set_unlocked(Some(keys));
        Ok(RecoveryKeyResult {
            recovery_key: recovery.to_string(),
        })
    }

    /// `backup_encryption_confirm`: the user saved the recovery key; the vault becomes the backup vault and new
    /// snapshots are sealed.
    pub fn backup_encryption_confirm(&self, _a: NoArgs) -> ApiResult<BackupEncryption> {
        let pending = pending_vault_path(&self.dirs);
        if load_vault(&pending)
            .map_err(|m| ApiError::new("invalid", m))?
            .is_none()
        {
            return Err(ApiError::new(
                "invalid",
                "there is no encryption to confirm",
            ));
        }
        std::fs::rename(&pending, vault_path(&self.dirs)).map_err(notatnik_graph::Error::from)?;
        let mut dev = notatnik_graph::DeviceSettings::load(&self.dirs)?;
        dev.ui.backup_encrypt = true;
        dev.save(&self.dirs)?;
        Ok(self.encryption_status())
    }

    /// `backup_unlock`: the password or the recovery key; the keys stay in memory for reading encrypted snapshots.
    pub fn backup_unlock(&self, a: BackupUnlockArgs) -> ApiResult<BackupEncryption> {
        let vault = self.configured_vault()?;
        let keys = vault
            .unlock(&a.secret)
            .map_err(|e| ApiError::new("invalid", e.to_string()))?;
        self.set_unlocked(Some(keys));
        Ok(self.encryption_status())
    }

    /// `backup_lock`: forgets the keys.
    pub fn backup_lock(&self, _a: NoArgs) -> ApiResult<BackupEncryption> {
        self.set_unlocked(None);
        Ok(self.encryption_status())
    }

    /// `backup_encryption_change_password`: rewraps the keys; snapshots already made keep opening with the old
    /// password (they carry the descriptor they were made with) and with the recovery key.
    pub fn backup_encryption_change_password(
        &self,
        a: BackupChangePasswordArgs,
    ) -> ApiResult<BackupEncryption> {
        Self::check_new_password(&a.new_password)?;
        let mut vault = self.configured_vault()?;
        let keys = vault
            .unlock(&a.old_password)
            .map_err(|e| ApiError::new("invalid", e.to_string()))?;
        vault
            .change_password(&keys, &a.new_password, notatnik_crypt::KdfParams::default())
            .map_err(|e| ApiError::new("internal", e.to_string()))?;
        save_vault(&vault_path(&self.dirs), &vault).map_err(notatnik_graph::Error::from)?;
        self.set_unlocked(Some(keys));
        Ok(self.encryption_status())
    }

    /// `backup_encryption_new_recovery_key`: replaces the recovery key; the old one stops opening new snapshots.
    pub fn backup_encryption_new_recovery_key(
        &self,
        a: BackupPasswordArgs,
    ) -> ApiResult<RecoveryKeyResult> {
        let mut vault = self.configured_vault()?;
        let keys = vault
            .unlock(&a.password)
            .map_err(|e| ApiError::new("invalid", e.to_string()))?;
        let recovery = vault
            .new_recovery_key(&keys)
            .map_err(|e| ApiError::new("internal", e.to_string()))?;
        save_vault(&vault_path(&self.dirs), &vault).map_err(notatnik_graph::Error::from)?;
        self.set_unlocked(Some(keys));
        Ok(RecoveryKeyResult {
            recovery_key: recovery.to_string(),
        })
    }

    /// `backup_encrypt_existing`: seals the plain snapshots in the backup folder and the second folder, then
    /// removes the plain files.
    pub fn backup_encrypt_existing(&self, _a: NoArgs) -> ApiResult<BackupEncryption> {
        let vault = self.configured_vault()?;
        self.with_open(|o| {
            let dev = notatnik_graph::DeviceSettings::load(&self.dirs).unwrap_or_default();
            let mut dirs = vec![primary_dir(&self.dirs, &dev)];
            dirs.extend(mirror_dir(&dev).filter(|m| m.is_dir()));
            for d in dirs {
                notatnik_graph::seal_existing(&d, &o.graph.id(), &vault)?;
            }
            Ok(())
        })?;
        Ok(self.encryption_status())
    }

    /// `list_backups`
    pub fn list_backups(&self, _a: NoArgs) -> ApiResult<BackupList> {
        self.with_open(|o| {
            let (err, mirror) = o
                .backup
                .as_ref()
                .map_or((None, None), |b| (take(&b.error), take(&b.mirror_error)));
            Ok(self.backup_list(&o.graph, err, mirror))
        })
    }

    /// `backup_now`: always takes a snapshot.
    pub fn backup_now(&self, _a: NoArgs) -> ApiResult<BackupList> {
        self.with_open(|o| {
            let (made, mirror) = run_backup(&self.dirs, &o.graph, true, None, self.unlocked_keys());
            made?;
            if let Some(b) = &o.backup {
                put(&b.error, None);
                put(&b.mirror_error, mirror.clone());
            }
            Ok(self.backup_list(&o.graph, None, mirror))
        })
    }

    /// `restore_backup`: unpacks a snapshot into a new or empty folder.
    pub fn restore_backup(&self, a: RestoreBackupArgs) -> ApiResult<StringValue> {
        let target = PathBuf::from(a.target.trim());
        if !target.is_absolute() {
            return Err(ApiError::new(
                "invalid",
                "the folder must be an absolute path",
            ));
        }
        self.with_open(|o| {
            let opts = self.read_options(&o.graph.id())?;
            o.graph
                .restore_backup_to(&self.backup_dir(), &a.name, &target, &opts)?;
            log::info!("backup {} restored to {}", a.name, target.display());
            Ok(StringValue {
                value: target.to_string_lossy().into_owned(),
            })
        })
    }

    /// `restore_backup_file`: unpacks a snapshot file from anywhere into a new or empty folder. Nothing of this
    /// device is needed: the file brings the descriptor of its vault, the secret opens it.
    pub fn restore_backup_file(&self, a: RestoreBackupFileArgs) -> ApiResult<StringValue> {
        let (file, target) = (PathBuf::from(a.file.trim()), PathBuf::from(a.target.trim()));
        if !file.is_absolute() || !target.is_absolute() {
            return Err(ApiError::new("invalid", "give absolute paths"));
        }
        if !file.is_file() {
            return Err(ApiError::new(
                "not_found",
                format!("{} is not a file", file.display()),
            ));
        }
        let n = notatnik_graph::restore_backup_file(&file, &target, &a.secret)?;
        log::info!("backup file restored: {n} files to {}", target.display());
        Ok(StringValue {
            value: target.to_string_lossy().into_owned(),
        })
    }

    /// The device settings stored in a snapshot (None for a snapshot without them).
    fn snapshot_settings(&self, name: &str) -> ApiResult<Option<String>> {
        if name.contains(['/', '\\'])
            || name.contains("..")
            || !(name.ends_with(".zip") || name.ends_with(".zip.enc"))
        {
            return Err(ApiError::new("invalid", "not a backup name"));
        }
        let entry = format!("{}/settings.toml", notatnik_graph::DEVICE_DIR);
        Ok(notatnik_graph::read_entry(
            &self.backup_dir().join(name),
            &entry,
            self.unlocked_keys().as_deref(),
        )?)
    }

    /// `backup_compare`: every file that differs between the snapshot and the graph (and device settings) as they
    /// are now (BK-10). Nothing changes.
    pub fn backup_compare(&self, a: BackupNameArgs) -> ApiResult<BackupCompare> {
        use notatnik_graph::ChangeStatus;
        self.with_open(|o| {
            let dev = notatnik_graph::DeviceSettings::load(&self.dirs).unwrap_or_default();
            let opts = self.read_options(&o.graph.id())?;
            let changes = o
                .graph
                .compare_backup(&primary_dir(&self.dirs, &dev), &a.name, &opts)?
                .into_iter()
                .map(|c| BackupChange {
                    path: c.path,
                    status: match c.status {
                        ChangeStatus::Changed => "changed",
                        ChangeStatus::OnlySnapshot => "only_snapshot",
                        ChangeStatus::OnlyNow => "only_now",
                        ChangeStatus::Locked => "locked",
                    }
                    .into(),
                    binary: c.binary,
                    added: c.added as u32,
                    removed: c.removed as u32,
                })
                .collect();
            Ok(BackupCompare { changes })
        })
    }

    /// `backup_file_diff`: the differing lines of one file.
    pub fn backup_file_diff(&self, a: BackupFileArgs) -> ApiResult<BackupFileDiff> {
        self.with_open(|o| {
            let dev = notatnik_graph::DeviceSettings::load(&self.dirs).unwrap_or_default();
            let opts = self.read_options(&o.graph.id())?;
            let d = o.graph.backup_file_diff(
                &primary_dir(&self.dirs, &dev),
                &a.name,
                &a.path,
                &opts,
            )?;
            Ok(BackupFileDiff {
                binary: d.binary,
                too_large: d.too_large,
                lines: d.lines.into_iter().map(diff_line).collect(),
            })
        })
    }

    /// `backup_device_preview`: how the device settings in the snapshot differ from the current file (BK-8).
    pub fn backup_device_preview(&self, a: BackupNameArgs) -> ApiResult<DeviceRestorePreview> {
        self.with_open(|_| {
            let Some(old) = self.snapshot_settings(&a.name)? else {
                return Ok(DeviceRestorePreview {
                    available: false,
                    same: true,
                    lines: Vec::new(),
                });
            };
            let now =
                std::fs::read_to_string(self.dirs.config.join("settings.toml")).unwrap_or_default();
            let lines: Vec<DiffLine> = notatnik_graph::diff_lines(&now, &old, 0)
                .unwrap_or_default()
                .into_iter()
                .filter(|o| !matches!(o, notatnik_graph::DiffOp::Gap))
                .map(diff_line)
                .collect();
            Ok(DeviceRestorePreview {
                available: true,
                same: lines.is_empty(),
                lines,
            })
        })
    }

    /// `restore_device_settings`: the snapshot's `settings.toml` replaces the current one. The current file is kept
    /// next to it as `settings.toml.before-restore-<time>` (BK-8). The graph history and all device settings come
    /// back as they were; nothing in a graph changes.
    pub fn restore_device_settings(&self, a: BackupNameArgs) -> ApiResult<SettingsView> {
        self.with_open(|o| {
            let old = self
                .snapshot_settings(&a.name)?
                .ok_or_else(|| ApiError::new("not_found", "the snapshot has no device settings"))?;
            notatnik_graph::DeviceSettings::parse(&old)
                .map_err(|e| ApiError::new("invalid", format!("settings in the snapshot: {e}")))?;
            let file = self.dirs.config.join("settings.toml");
            if file.exists() {
                let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
                std::fs::copy(
                    &file,
                    self.dirs
                        .config
                        .join(format!("settings.toml.before-restore-{stamp}")),
                )
                .map_err(notatnik_graph::Error::from)?;
            }
            std::fs::create_dir_all(&self.dirs.config).map_err(notatnik_graph::Error::from)?;
            std::fs::write(&file, old).map_err(notatnik_graph::Error::from)?;
            self.settings_view(o)
        })
    }

    /// `restore_backup_page`: the page's file in the snapshot replaces the current one.
    pub fn restore_backup_page(&self, a: RestoreBackupPageArgs) -> ApiResult<StringList> {
        let name = Self::parse_name(&a.page)?;
        self.with_open(|o| {
            let rel = page_to_path(&name);
            let dev = notatnik_graph::DeviceSettings::load(&self.dirs).unwrap_or_default();
            let mut opts =
                backup_options(&self.dirs, &dev, &o.graph.id(), self.unlocked_keys(), true)
                    .map_err(|m| ApiError::new("invalid", m))?;
            opts.graph_keys = self.graph_keys_now();
            o.graph.restore_page_from_backup(
                &primary_dir(&self.dirs, &dev),
                &a.name,
                &rel,
                chrono::Local::now().naive_local(),
                &opts,
            )?;
            // undo cannot follow a replaced file
            o.undo.retain(|e| e.rel != rel);
            o.redo.retain(|e| e.rel != rel);
            (self.emit)(
                "graph-changed",
                json!({ "pages": [name.to_string()], "paths": [rel], "own": true }),
            );
            Ok(StringList {
                items: vec![name.to_string()],
            })
        })
    }
}

/// One item of a YAML flow list: plain when that is safe, otherwise in double quotes.
fn yaml_item(s: &str) -> String {
    let plain = !s.is_empty()
        && s == s.trim()
        && !s.starts_with(['-', '?', '!', '&', '*', '|', '>', '%', '@', '`', '\'', '"'])
        && !s.contains([',', '[', ']', '{', '}', '#', '"', '\\'])
        && !s.contains(": ")
        && !s.ends_with(':');
    if plain {
        s.to_string()
    } else {
        format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
    }
}

/// The MIME type of an asset file, from its extension. The shell and the dev server serve assets with it.
pub fn asset_content_type(path: &str) -> &'static str {
    match path
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "webp" => "image/webp",
        "pdf" => "application/pdf",
        "txt" | "md" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}
