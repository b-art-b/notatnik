//! File watcher: debounced, self-write suppression by content hash (P1.32).
//!
//! After a quiet period the watcher looks at the *final state* of every touched path: a page file
//! is reindexed only if its content hash differs from the one in the index. Our own writes update
//! the index before the watcher fires, so they are ignored; a temp-file-plus-rename save is seen as
//! one modification of the target.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use notify::{RecursiveMode, Watcher as _};

use crate::{Error, Graph, Result, hash_bytes};
use notatnik_index::{DuplicateId, walk};

/// Pages changed by something other than this process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphChanged {
    /// Names of changed pages (sorted, unique).
    pub pages: Vec<String>,
    /// Their file paths relative to the graph root.
    pub paths: Vec<String>,
}

/// A running watcher; dropping it stops the thread.
pub struct Watcher {
    _watcher: notify::RecommendedWatcher,
    rx: Receiver<GraphChanged>,
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl Watcher {
    /// Channel of change events.
    pub fn events(&self) -> &Receiver<GraphChanged> {
        &self.rx
    }
}

impl Drop for Watcher {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

fn is_hidden(rel: &Path) -> bool {
    rel.components()
        .any(|c| c.as_os_str().to_string_lossy().starts_with('.'))
}

impl Graph {
    /// Starts watching the graph folder. Events are debounced by `debounce` (≈200 ms in the app) and
    /// delivered on [`Watcher::events`].
    pub fn watch(&self, debounce: Duration) -> Result<Watcher> {
        let (tx, rx) = mpsc::channel::<GraphChanged>();
        let mut w = self.watch_with(debounce, move |ev| {
            let _ = tx.send(ev);
        })?;
        w.rx = rx;
        Ok(w)
    }

    /// Like [`Graph::watch`], but calls `sink` (on the watcher thread) for every change set.
    pub fn watch_with(
        &self,
        debounce: Duration,
        sink: impl Fn(GraphChanged) + Send + 'static,
    ) -> Result<Watcher> {
        let (raw_tx, raw_rx) = mpsc::channel::<notify::Result<notify::Event>>();
        let mut watcher = notify::recommended_watcher(move |res| {
            let _ = raw_tx.send(res);
        })
        .map_err(|e| Error::Io(std::io::Error::other(e)))?;
        watcher
            .watch(self.root(), RecursiveMode::Recursive)
            .map_err(|e| Error::Io(std::io::Error::other(e)))?;
        let (_unused_tx, rx) = mpsc::channel::<GraphChanged>();
        let stop = Arc::new(AtomicBool::new(false));
        let graph = self.clone();
        let stop2 = stop.clone();
        let handle = std::thread::spawn(move || {
            let mut pending: HashSet<PathBuf> = HashSet::new();
            let mut last = Instant::now();
            while !stop2.load(Ordering::Relaxed) {
                match raw_rx.recv_timeout(Duration::from_millis(25)) {
                    Ok(Ok(ev)) => {
                        use notify::EventKind::{Access, Other};
                        if !matches!(ev.kind, Access(_) | Other) {
                            pending.extend(ev.paths);
                            last = Instant::now();
                        }
                    }
                    Ok(Err(_)) | Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => break,
                }
                if !pending.is_empty() && last.elapsed() >= debounce {
                    let paths: Vec<PathBuf> = pending.drain().collect();
                    if let Some(ev) = graph.process_events(&paths) {
                        sink(ev);
                    }
                }
            }
        });
        Ok(Watcher {
            _watcher: watcher,
            rx,
            stop,
            handle: Some(handle),
        })
    }

    /// Looks at the final state of the touched paths and updates the index. Returns the change set.
    fn process_events(&self, paths: &[PathBuf]) -> Option<GraphChanged> {
        let root = self.root().to_path_buf();
        let mut changed: Vec<String> = Vec::new();
        let mut dups: Vec<(String, Vec<DuplicateId>)> = Vec::new();
        let mut full = false;
        let mut conflict_paths: Vec<String> = Vec::new();
        // `.notatnik/config.toml` edited by hand (or synced from another device): read it again
        let config_touched = paths.iter().any(|p| {
            p.strip_prefix(&root)
                .is_ok_and(|r| r == Path::new(".notatnik/config.toml"))
        });
        let config_changed = config_touched && self.reload_config();
        {
            let mut g = self.lock();
            let mut files: Vec<String> = Vec::new();
            for p in paths {
                let Ok(rel) = p.strip_prefix(&root) else {
                    continue;
                };
                if is_hidden(rel) {
                    continue;
                }
                let rel_s = rel.to_string_lossy().replace('\\', "/");
                let name = p
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                if p.is_dir() {
                    full = true;
                    continue;
                }
                if !name.ends_with(".md") {
                    if !p.exists() {
                        // a removed directory? (only matters if pages lived below it)
                        let prefix = format!("{rel_s}/");
                        if g.index
                            .page_files()
                            .map(|v| v.iter().any(|(_, f)| f.starts_with(&prefix)))
                            .unwrap_or(false)
                        {
                            full = true;
                        }
                    }
                    continue;
                }
                if walk::is_conflict_name(&name) {
                    conflict_paths.push(rel_s);
                    continue;
                }
                if notatnik_core::path_to_page(&rel_s).is_none() {
                    continue;
                }
                files.push(rel_s);
            }
            files.sort();
            files.dedup();
            for rel in files {
                let abs = root.join(&rel);
                let indexed = g.index.file_hash(&rel).ok().flatten();
                if abs.is_file() {
                    let Ok(bytes) = std::fs::read(&abs) else {
                        continue;
                    };
                    if indexed.as_deref() == Some(hash_bytes(&bytes).as_str()) {
                        continue;
                    }
                    g.reindexes.fetch_add(1, Ordering::Relaxed);
                    if let Ok(out) = g.index.reindex_file(&root, &rel) {
                        g.snapshot_file(&rel);
                        if !out.duplicates.is_empty() {
                            dups.push((rel.clone(), out.duplicates));
                        }
                        changed.push(rel);
                    }
                } else if indexed.is_some() {
                    g.reindexes.fetch_add(1, Ordering::Relaxed);
                    if g.index.remove_file(&rel).is_ok() {
                        g.remove_snapshot(&rel);
                        changed.push(rel);
                    }
                }
            }
            if full && let Ok(s) = g.index.reconcile(&root) {
                for rel in s.reindexed_paths.iter().chain(&s.deleted_paths) {
                    changed.push(rel.clone());
                }
                for rel in &s.reindexed_paths {
                    g.snapshot_file(rel);
                }
                dups.extend(s.duplicates);
            }
        }
        for (rel, d) in dups {
            let _ = self.fix_duplicates(&rel, &d);
        }
        if changed.is_empty() {
            if config_changed {
                return Some(GraphChanged {
                    pages: Vec::new(),
                    paths: vec![".notatnik/config.toml".to_string()],
                });
            }
            // conflict copies aren't pages, but the UI wants to know that the list changed
            if conflict_paths.is_empty() {
                return None;
            }
            conflict_paths.sort();
            conflict_paths.dedup();
            log::warn!(
                "conflict copy appeared (merge it by hand): {}",
                conflict_paths.join(", ")
            );
            return Some(GraphChanged {
                pages: Vec::new(),
                paths: conflict_paths,
            });
        }
        if config_changed {
            changed.push(".notatnik/config.toml".to_string());
        }
        changed.sort();
        changed.dedup();
        let mut pages: Vec<String> = changed
            .iter()
            .filter_map(|p| notatnik_core::path_to_page(p))
            .map(|n| n.to_string())
            .collect();
        pages.sort();
        pages.dedup();
        Some(GraphChanged {
            pages,
            paths: changed,
        })
    }
}
