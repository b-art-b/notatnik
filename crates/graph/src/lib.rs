//! The graph service: opening a graph folder, atomic guarded writes, the file watcher,
//! reconciliation at startup, rename with reference rewriting, conflict-file detection, history snapshots.
//!
//! Safety rules (docs/testing.md §7): a page file is only ever replaced atomically and only if it
//! still has the content the caller based its edit on; nothing but synced content lives inside the
//! graph folder except `.notatnik/` (config and temp files).

pub mod backup;
mod config;
mod error;
mod rename;
pub mod synth;
mod watch;

pub use backup::BackupFile;
pub use config::{
    CalendarSection, Config, DATE_FORMATS, DeviceSettings, Dirs, GraphEntry, GraphSection,
    NamespaceSection, PageIconRule, PropertiesSection, SavedQuery, TasksSection,
    default_priorities, new_graph_id, validate_priorities,
};
pub use error::{Error, Result};
pub use rename::{FileEdit, RenamePlan, RenameReport};
pub use watch::{GraphChanged, Watcher};

use std::collections::HashSet;
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use notatnik_core::{BlockOp, EditError, TextPatch};
use notatnik_index::{DuplicateId, Index, IndexOptions, IssueRow, ReconcileStats, walk};

pub(crate) struct Inner {
    pub(crate) root: PathBuf,
    pub(crate) config: Config,
    pub(crate) index: Index,
    pub(crate) data_dir: PathBuf,
    pub(crate) reindexes: AtomicUsize,
}

/// A graph folder opened for use. Cheap to clone; clones share the same state.
#[derive(Clone)]
pub struct Graph {
    pub(crate) inner: Arc<Mutex<Inner>>,
    root: PathBuf,
}

/// What `open` did.
#[derive(Debug, Clone, Default)]
pub struct OpenReport {
    /// Warnings about the config (unknown keys, bad values).
    pub warnings: Vec<String>,
    /// The index had an old schema version and was rebuilt from scratch.
    pub index_rebuilt: bool,
    /// Counters of the startup reconciliation (all files count as added on a rebuild).
    pub reconcile: ReconcileStats,
    /// Block IDs that were duplicated and got a new ID written to their file.
    pub reids: Vec<ReId>,
}

/// A duplicate block ID that was replaced.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ReId {
    /// File that was changed.
    pub path: String,
    /// The duplicated ID.
    pub old: String,
    /// The new ID.
    pub new: String,
}

/// A page file's text and hash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageFile {
    /// File content.
    pub text: String,
    /// blake3 hex of the bytes: pass back as `base_hash` when writing.
    pub hash: String,
}

/// Result of a write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteOutcome {
    /// Hash of the new file content (after any duplicate-ID fix).
    pub hash: String,
}

/// Result of `edit_page`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditOutcome {
    /// Hash of the new file content.
    pub hash: String,
    /// The new text.
    pub new_text: String,
    /// Patches against the text the edit was based on, for undo (ED-6).
    pub patches: Vec<TextPatch>,
}

/// Health check result (`notatnik check`).
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct CheckReport {
    /// Parse diagnostics and other per-file issues.
    pub issues: Vec<IssueRow>,
    /// Nextcloud conflict copies.
    pub conflicts: Vec<String>,
    /// `.md` files that don't map to a page.
    pub unmapped: Vec<String>,
    /// Links to pages that have no file (informational: they are virtual pages).
    pub unresolved_links: Vec<(String, String)>,
    /// `((id))` references whose target doesn't exist.
    pub dangling_block_refs: Vec<(String, String, String)>,
    /// Block IDs that were replaced on this device.
    pub reids: Vec<ReId>,
}

impl CheckReport {
    /// True when something needs attention (exit code 2). Unresolved page links and re-IDs are informational.
    pub fn has_problems(&self) -> bool {
        self.issues.iter().any(|i| i.severity != "info")
            || !self.conflicts.is_empty()
            || !self.unmapped.is_empty()
            || !self.dangling_block_refs.is_empty()
    }
}

pub(crate) fn hash_bytes(b: &[u8]) -> String {
    blake3::hash(b).to_hex().to_string()
}

pub(crate) fn sync_dir(dir: &Path) {
    if let Ok(d) = std::fs::File::open(dir) {
        let _ = d.sync_all();
    }
}

/// Writes `bytes` to `abs` atomically: temp file in `.notatnik/tmp/`, fsync, rename, fsync the directory.
pub(crate) fn atomic_write(root: &Path, abs: &Path, bytes: &[u8]) -> Result<()> {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let tmp_dir = root.join(".notatnik").join("tmp");
    std::fs::create_dir_all(&tmp_dir)?;
    let tmp = tmp_dir.join(format!(
        "w{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let result = (|| -> Result<()> {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        if let Ok(md) = std::fs::metadata(abs) {
            let _ = std::fs::set_permissions(&tmp, md.permissions());
        }
        if let Some(parent) = abs.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::rename(&tmp, abs)?;
        if let Some(parent) = abs.parent() {
            sync_dir(parent);
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

fn check_rel(rel: &str) -> Result<()> {
    let p = Path::new(rel);
    if p.is_absolute()
        || p.components()
            .any(|c| matches!(c, Component::ParentDir | Component::CurDir))
    {
        return Err(Error::InvalidPath(rel.into()));
    }
    if notatnik_core::path_to_page(rel).is_none() {
        return Err(Error::InvalidPath(rel.into()));
    }
    Ok(())
}

fn xorshift_seed(salt: &str) -> u64 {
    static COUNTER: AtomicU64 = AtomicU64::new(1);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let h = blake3::hash(
        format!(
            "{nanos}-{}-{salt}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        )
        .as_bytes(),
    );
    let mut b = [0u8; 8];
    b.copy_from_slice(&h.as_bytes()[..8]);
    u64::from_le_bytes(b) | 1
}

impl Graph {
    /// Creates a new graph folder (`pages/`, `journals/`, `.notatnik/config.toml` with a fresh ID) and opens it.
    pub fn create(path: &Path, dirs: &Dirs) -> Result<(Graph, OpenReport)> {
        std::fs::create_dir_all(path.join("pages"))?;
        std::fs::create_dir_all(path.join("journals"))?;
        Graph::open(path, dirs)
    }

    /// Opens a graph folder. Folders not created by notatnik work too: `.notatnik/` is created and
    /// the files are indexed. Nothing else is created inside the folder.
    pub fn open(path: &Path, dirs: &Dirs) -> Result<(Graph, OpenReport)> {
        let root =
            std::fs::canonicalize(path).map_err(|_| Error::NotFound(path.display().to_string()))?;
        if !root.is_dir() {
            return Err(Error::NotFound(path.display().to_string()));
        }
        let meta = root.join(".notatnik");
        std::fs::create_dir_all(meta.join("tmp"))?;
        let cfg_path = meta.join("config.toml");
        let (mut config, warnings) = match std::fs::read_to_string(&cfg_path) {
            Ok(t) => Config::parse(&t)?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (Config::default(), Vec::new()),
            Err(e) => return Err(e.into()),
        };
        if config.graph.id.is_empty() {
            config.graph.id = new_graph_id();
            atomic_write(&root, &cfg_path, config.to_toml().as_bytes())?;
        }
        // leftover temp files from an interrupted write
        if let Ok(rd) = std::fs::read_dir(meta.join("tmp")) {
            for e in rd.flatten() {
                let _ = std::fs::remove_file(e.path());
            }
        }
        let data_dir = dirs.graph_data(&config.graph.id);
        let options = IndexOptions {
            property_types: config.property_types(),
            priority_labels: config.priority_labels(),
        };
        let (mut index, info) = Index::open(&data_dir.join("index.sqlite"), options)?;
        let mut report = OpenReport {
            warnings,
            index_rebuilt: info.rebuilt,
            ..OpenReport::default()
        };
        let counts = index.counts()?;
        let fresh = counts.pages == 0 && counts.virtual_pages == 0;
        let dups: Vec<(String, Vec<DuplicateId>)> = if info.rebuilt || fresh {
            let d = index.index_all(&root)?;
            let n = index.counts()?.pages;
            report.reconcile.reindexed = n;
            report.reconcile.added = n;
            d
        } else {
            let stats = index.reconcile(&root)?;
            let d = stats.duplicates.clone();
            report.reconcile = stats;
            d
        };
        let mut settings = DeviceSettings::load(dirs).unwrap_or_default();
        settings.register(&config.graph.id, &root);
        let _ = settings.save(dirs);

        let graph = Graph {
            inner: Arc::new(Mutex::new(Inner {
                root: root.clone(),
                config,
                index,
                data_dir,
                reindexes: AtomicUsize::new(0),
            })),
            root,
        };
        // history snapshots for everything that was (re)indexed
        let paths: Vec<String> = if info.rebuilt || fresh {
            walk::scan(&graph.root)?
                .pages
                .into_iter()
                .map(|f| f.rel)
                .collect()
        } else {
            report.reconcile.reindexed_paths.clone()
        };
        {
            let g = graph.lock();
            for rel in &paths {
                g.snapshot_file(rel);
            }
            for rel in &report.reconcile.deleted_paths {
                g.remove_snapshot(rel);
            }
        }
        for (rel, d) in dups {
            report.reids.extend(graph.fix_duplicates(&rel, &d)?);
        }
        Ok((graph, report))
    }

    pub(crate) fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// The graph folder (absolute).
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Changes `config.toml` (atomic write) and the in-memory config.
    pub fn update_config(&self, f: impl FnOnce(&mut Config)) -> Result<()> {
        let mut g = self.lock();
        let mut cfg = g.config.clone();
        f(&mut cfg);
        let path = g.root.join(".notatnik").join("config.toml");
        let root = g.root.clone();
        atomic_write(&root, &path, cfg.to_toml().as_bytes())?;
        // the priority labels decide how `priority::` parses: files are indexed again when they change
        let relabel = g.index.priority_labels() != cfg.priority_labels().as_slice();
        g.config = cfg;
        if relabel {
            let options = IndexOptions {
                property_types: g.config.property_types(),
                priority_labels: g.config.priority_labels(),
            };
            g.index.set_options(options);
            g.index.index_all(&root)?;
        }
        Ok(())
    }

    /// Reads `config.toml` again (it was edited outside the app). Returns whether the settings changed. A file that
    /// does not parse is ignored: the settings in use stay. The priority labels decide how files are parsed, so a
    /// change of them re-reads everything.
    pub fn reload_config(&self) -> bool {
        let mut g = self.lock();
        let path = g.root.join(".notatnik").join("config.toml");
        let Ok(text) = std::fs::read_to_string(&path) else {
            return false;
        };
        let Ok((mut cfg, _)) = Config::parse(&text) else {
            return false;
        };
        if cfg.graph.id.is_empty() {
            cfg.graph.id = g.config.graph.id.clone();
        }
        if cfg == g.config {
            return false;
        }
        let relabel = g.index.priority_labels() != cfg.priority_labels().as_slice();
        g.config = cfg;
        if relabel {
            let options = IndexOptions {
                property_types: g.config.property_types(),
                priority_labels: g.config.priority_labels(),
            };
            let root = g.root.clone();
            g.index.set_options(options);
            let _ = g.index.index_all(&root);
        }
        true
    }

    /// A copy of the current config.
    pub fn config(&self) -> Config {
        self.lock().config.clone()
    }

    /// The graph ID.
    pub fn id(&self) -> String {
        self.lock().config.graph.id.clone()
    }

    /// Runs `f` with read access to the index.
    pub fn with_index<R>(&self, f: impl FnOnce(&Index) -> R) -> R {
        f(&self.lock().index)
    }

    /// Runs `f` with mutable access to the index (fuzzy search keeps a cache).
    pub fn with_index_mut<R>(&self, f: impl FnOnce(&mut Index) -> R) -> R {
        f(&mut self.lock().index)
    }

    /// How many times the watcher reindexed a file (tests, diagnostics).
    pub fn watcher_reindex_count(&self) -> usize {
        self.lock().reindexes.load(Ordering::Relaxed)
    }

    /// Makes the index agree with the file on disk right now (the watcher is debounced, so a file that was
    /// just created or edited may not be indexed yet). Returns whether a reindex happened.
    pub fn refresh_file(&self, rel: &str, hash: &str) -> Result<bool> {
        let mut g = self.lock();
        if g.index.file_hash(rel)?.as_deref() == Some(hash) {
            return Ok(false);
        }
        let root = g.root.clone();
        g.index.reindex_file(&root, rel)?;
        g.snapshot_file(rel);
        Ok(true)
    }

    /// Reads a page file (path relative to the graph root).
    pub fn read_page(&self, rel: &str) -> Result<PageFile> {
        let bytes = std::fs::read(self.root.join(rel)).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                Error::NotFound(rel.into())
            } else {
                Error::Io(e)
            }
        })?;
        let hash = hash_bytes(&bytes);
        let text = String::from_utf8(bytes)
            .map_err(|_| Error::InvalidPath(format!("{rel} is not valid UTF-8")))?;
        Ok(PageFile { text, hash })
    }

    /// Writes a page file atomically, but only if it still has the content with hash `base_hash`
    /// (`None` = the file must not exist yet). Otherwise `Err(Conflict)` and the file is untouched.
    pub fn write_page(
        &self,
        rel: &str,
        new_text: &str,
        base_hash: Option<&str>,
    ) -> Result<WriteOutcome> {
        check_rel(rel)?;
        let outcome = self.write_checked(rel, new_text, base_hash)?;
        // duplicate block ids introduced by this write are fixed right away
        let dups = {
            let mut g = self.lock();
            let root = g.root.clone();
            g.index.reindex_file(&root, rel)?.duplicates
        };
        if dups.is_empty() {
            return Ok(outcome);
        }
        self.fix_duplicates(rel, &dups)?;
        let hash = self.read_page(rel)?.hash;
        Ok(WriteOutcome { hash })
    }

    fn write_checked(
        &self,
        rel: &str,
        new_text: &str,
        base_hash: Option<&str>,
    ) -> Result<WriteOutcome> {
        let abs = self.root.join(rel);
        let current = match std::fs::read(&abs) {
            Ok(b) => Some(hash_bytes(&b)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(e.into()),
        };
        match (current.as_deref(), base_hash) {
            (Some(c), Some(b)) if c == b => {}
            (None, None) => {}
            _ => return Err(Error::Conflict { path: rel.into() }),
        }
        atomic_write(&self.root, &abs, new_text.as_bytes())?;
        self.lock().snapshot_file(rel);
        Ok(WriteOutcome {
            hash: hash_bytes(new_text.as_bytes()),
        })
    }

    /// Deletes a page file if it still has the content with hash `base_hash` (used by undo of a page creation).
    pub fn delete_page(&self, rel: &str, base_hash: &str) -> Result<()> {
        check_rel(rel)?;
        let abs = self.root.join(rel);
        let bytes = std::fs::read(&abs).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                Error::NotFound(rel.into())
            } else {
                Error::Io(e)
            }
        })?;
        if hash_bytes(&bytes) != base_hash {
            return Err(Error::Conflict { path: rel.into() });
        }
        std::fs::remove_file(&abs)?;
        if let Some(parent) = abs.parent() {
            sync_dir(parent);
            // tidy empty folders below pages/
            let mut dir = Some(parent.to_path_buf());
            while let Some(d) = dir {
                if d == self.root.join("pages")
                    || d == self.root.join("journals")
                    || !d.starts_with(&self.root)
                    || std::fs::remove_dir(&d).is_err()
                {
                    break;
                }
                dir = d.parent().map(Path::to_path_buf);
            }
        }
        let mut g = self.lock();
        let root = g.root.clone();
        g.index.reindex_file(&root, rel)?;
        g.remove_snapshot(rel);
        Ok(())
    }

    /// Moves page files to the trash: each is copied below `<data dir>/trash/<id>/` (outside the graph folder, so it
    /// is never synced) and then deleted like [`Graph::delete_page`]. Returns the trash id for
    /// [`Graph::restore_trash`]. Trash older than 30 days is removed on the way.
    pub fn trash_pages(&self, rels: &[String]) -> Result<String> {
        let (data_dir, root) = {
            let g = self.lock();
            (g.data_dir.clone(), g.root.clone())
        };
        let trash = data_dir.join("trash");
        let id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis())
            .to_string();
        // read everything first: if one file is unreadable nothing was touched
        let mut files = Vec::new();
        for rel in rels {
            check_rel(rel)?;
            let bytes = std::fs::read(root.join(rel)).map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    Error::NotFound(rel.clone())
                } else {
                    Error::Io(e)
                }
            })?;
            files.push((rel.clone(), bytes));
        }
        for (rel, bytes) in &files {
            let dest = trash.join(&id).join(rel);
            if let Some(dir) = dest.parent() {
                std::fs::create_dir_all(dir)?;
            }
            std::fs::write(&dest, bytes)?;
        }
        for (rel, bytes) in &files {
            self.delete_page(rel, &hash_bytes(bytes))?;
        }
        // retention
        let cutoff = std::time::SystemTime::now()
            .checked_sub(std::time::Duration::from_secs(30 * 24 * 3600))
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_millis());
        if let Ok(rd) = std::fs::read_dir(&trash) {
            for e in rd.flatten() {
                let old = e
                    .file_name()
                    .to_str()
                    .and_then(|n| n.parse::<u128>().ok())
                    .is_some_and(|ms| ms < cutoff);
                if old {
                    let _ = std::fs::remove_dir_all(e.path());
                }
            }
        }
        Ok(id)
    }

    /// Puts a trash entry back: every file returns to its old path unless something else is there now (that file
    /// stays in the trash entry). Returns the restored paths; the entry is removed once all files are back.
    pub fn restore_trash(&self, id: &str) -> Result<Vec<String>> {
        if id.is_empty() || !id.bytes().all(|b| b.is_ascii_digit()) {
            return Err(Error::NotFound(format!("trash entry `{id}`")));
        }
        let (dir, root) = {
            let g = self.lock();
            (g.data_dir.join("trash").join(id), g.root.clone())
        };
        if !dir.is_dir() {
            return Err(Error::NotFound(format!("trash entry `{id}`")));
        }
        let mut stack = vec![dir.clone()];
        let mut found = Vec::new();
        while let Some(d) = stack.pop() {
            for e in std::fs::read_dir(&d)?.flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                } else if let Ok(rel) = p.strip_prefix(&dir) {
                    found.push(rel.to_string_lossy().replace('\\', "/"));
                }
            }
        }
        found.sort();
        let mut restored = Vec::new();
        for rel in &found {
            check_rel(rel)?;
            if root.join(rel).exists() {
                continue;
            }
            let text = std::fs::read_to_string(dir.join(rel))?;
            self.write_page(rel, &text, None)?; // also reindexes the file
            restored.push(rel.clone());
        }
        if restored.len() == found.len() {
            let _ = std::fs::remove_dir_all(&dir);
        }
        Ok(restored)
    }

    /// Applies edit operations to a page file: reads it, applies the ops in order (reparsing between
    /// them), and writes the result with the base-hash guard.
    ///
    /// `base_hash = None` means the file must not exist yet: the ops are applied to an empty page and
    /// the file is created (a virtual page becomes a file on its first content, LN-1).
    pub fn edit_page(
        &self,
        rel: &str,
        base_hash: Option<&str>,
        ops: &[BlockOp],
    ) -> Result<EditOutcome> {
        let (old_text, base) = match (self.read_page(rel), base_hash) {
            (Ok(file), Some(h)) if file.hash == h => (file.text, Some(h.to_string())),
            (Ok(_), _) => return Err(Error::Conflict { path: rel.into() }),
            (Err(Error::NotFound(_)), None) => (String::new(), None),
            (Err(Error::NotFound(_)), Some(_)) => return Err(Error::Conflict { path: rel.into() }),
            (Err(e), _) => return Err(e),
        };
        let opts = self.config().parse_options();
        let mut text = old_text.clone();
        for op in ops {
            let page = notatnik_core::parse_page_with(&text, &opts);
            text = notatnik_core::apply(&text, &page, op)
                .map_err(Error::Edit)?
                .new_source;
        }
        let patches = whole_diff(&old_text, &text);
        let out = self.write_page(rel, &text, base.as_deref())?;
        let final_text = self.read_page(rel)?.text;
        Ok(EditOutcome {
            hash: out.hash,
            new_text: final_text,
            patches,
        })
    }

    /// Replaces duplicated block IDs in `rel` with fresh ones (file-format.md §3.4) and logs them.
    pub(crate) fn fix_duplicates(&self, rel: &str, dups: &[DuplicateId]) -> Result<Vec<ReId>> {
        let mut file = self.read_page(rel)?;
        let mut used: HashSet<String> = self.with_index(|i| i.all_block_uids())?;
        let mut state = xorshift_seed(rel);
        let mut rng = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        let mut done = Vec::new();
        for d in dups {
            let new = notatnik_core::ids::generate_block_id(&used, &mut rng);
            used.insert(new.clone());
            let page = notatnik_core::parse_page_with(&file.text, &self.config().parse_options());
            match notatnik_core::replace_block_id(&file.text, &page, &d.path, &new) {
                Ok(r) => {
                    file.text = r.new_source;
                    done.push(ReId {
                        path: rel.to_string(),
                        old: d.id.clone(),
                        new,
                    });
                }
                Err(EditError::NoSuchBlock(_)) => {}
                Err(e) => return Err(e.into()),
            }
        }
        if done.is_empty() {
            return Ok(done);
        }
        self.write_checked(rel, &file.text, Some(&file.hash))?;
        {
            let mut g = self.lock();
            let root = g.root.clone();
            g.index.reindex_file(&root, rel)?;
            g.log_reids(&done);
        }
        Ok(done)
    }

    /// Re-runs the startup reconciliation (e.g. after the watcher dropped events).
    pub fn reconcile(&self) -> Result<ReconcileStats> {
        let (stats, dups) = {
            let mut g = self.lock();
            let root = g.root.clone();
            let s = g.index.reconcile(&root)?;
            for rel in &s.reindexed_paths {
                g.snapshot_file(rel);
            }
            for rel in &s.deleted_paths {
                g.remove_snapshot(rel);
            }
            let d = s.duplicates.clone();
            (s, d)
        };
        for (rel, d) in dups {
            self.fix_duplicates(&rel, &d)?;
        }
        Ok(stats)
    }

    /// Nextcloud conflict copies currently in the graph (not indexed as pages).
    pub fn conflict_files(&self) -> Result<Vec<String>> {
        Ok(walk::scan(&self.root)?.conflicts)
    }

    /// The health check.
    pub fn check(&self) -> Result<CheckReport> {
        let files = walk::scan(&self.root)?;
        let g = self.lock();
        Ok(CheckReport {
            issues: g.index.issues()?,
            conflicts: files.conflicts,
            unmapped: files.unmapped,
            unresolved_links: g.index.unresolved_links()?,
            dangling_block_refs: g.index.dangling_block_refs()?,
            reids: g.read_reids(),
        })
    }

    /// The saved copy of a page file from the last time we read or wrote it (the 3-way merge base).
    pub fn history_snapshot(&self, rel: &str) -> Option<String> {
        self.lock().read_snapshot(rel)
    }

    /// Total size of the history folder in bytes.
    pub fn history_size(&self) -> u64 {
        let dir = self.lock().data_dir.join("history");
        std::fs::read_dir(dir).map_or(0, |rd| {
            rd.flatten()
                .filter_map(|e| e.metadata().ok())
                .map(|m| m.len())
                .sum()
        })
    }

    /// Number of snapshot files in the history folder.
    pub fn history_count(&self) -> usize {
        let dir = self.lock().data_dir.join("history");
        std::fs::read_dir(dir).map_or(0, |rd| rd.flatten().count())
    }
}

impl Inner {
    fn snapshot_path(&self, rel: &str) -> PathBuf {
        let h = hash_bytes(rel.as_bytes());
        self.data_dir.join("history").join(&h[..32])
    }

    /// Stores the file's current content as the merge base. Failures are not fatal (it's a cache).
    pub(crate) fn snapshot_file(&self, rel: &str) {
        let Ok(bytes) = std::fs::read(self.root.join(rel)) else {
            return;
        };
        let p = self.snapshot_path(rel);
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let tmp = p.with_extension("tmp");
        if std::fs::write(&tmp, bytes).is_ok() {
            let _ = std::fs::rename(&tmp, &p);
        }
    }

    pub(crate) fn remove_snapshot(&self, rel: &str) {
        let _ = std::fs::remove_file(self.snapshot_path(rel));
    }

    fn read_snapshot(&self, rel: &str) -> Option<String> {
        std::fs::read_to_string(self.snapshot_path(rel)).ok()
    }

    fn log_reids(&self, reids: &[ReId]) {
        let _ = std::fs::create_dir_all(&self.data_dir);
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.data_dir.join("reid.log"))
        {
            for r in reids {
                if let Ok(line) = serde_json::to_string(r) {
                    let _ = writeln!(f, "{line}");
                }
            }
        }
    }

    fn read_reids(&self) -> Vec<ReId> {
        std::fs::read_to_string(self.data_dir.join("reid.log"))
            .map(|t| {
                t.lines()
                    .filter_map(|l| serde_json::from_str(l).ok())
                    .collect()
            })
            .unwrap_or_default()
    }
}

/// One patch covering the differing middle of two texts.
fn whole_diff(old: &str, new: &str) -> Vec<TextPatch> {
    if old == new {
        return Vec::new();
    }
    let prefix = old
        .char_indices()
        .zip(new.chars())
        .take_while(|((_, a), b)| a == b)
        .last()
        .map_or(0, |((i, a), _)| i + a.len_utf8());
    let max = old.len().min(new.len()) - prefix;
    let mut suffix = 0;
    for (a, b) in old.chars().rev().zip(new.chars().rev()) {
        if a != b || suffix + a.len_utf8() > max {
            break;
        }
        suffix += a.len_utf8();
    }
    vec![TextPatch {
        range: notatnik_core::Span::new(prefix, old.len() - suffix),
        old_text: old
            .get(prefix..old.len() - suffix)
            .unwrap_or("")
            .to_string(),
        new_text: new
            .get(prefix..new.len() - suffix)
            .unwrap_or("")
            .to_string(),
    }]
}
