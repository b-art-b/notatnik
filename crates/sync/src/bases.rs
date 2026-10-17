//! Copies of the pages as they were at the last sync (the common ancestor of a three-way merge, ST-6), kept
//! outside the replica next to the sync state (ST-2). Only Markdown files of a sensible size are kept.

use std::path::{Path, PathBuf};

/// The largest page that is kept for merging.
const MAX_BASE_BYTES: usize = 4 * 1024 * 1024;

/// Where the copies are. Without a folder every call does nothing.
#[derive(Debug, Clone, Default)]
pub struct Bases {
    dir: Option<PathBuf>,
}

impl Bases {
    /// Copies below `dir` (`synced/` for the last synced content, `ancestor/` for the common ancestor of a
    /// conflict copy).
    pub fn new(dir: Option<PathBuf>) -> Bases {
        Bases { dir }
    }

    fn file(&self, kind: &str, path: &str) -> Option<PathBuf> {
        let ok = path.ends_with(".md")
            && !path.starts_with('/')
            && !path.split('/').any(|c| c == ".." || c.is_empty());
        let dir = self.dir.as_ref().filter(|_| ok)?;
        Some(dir.join(kind).join(path))
    }

    fn save(&self, kind: &str, path: &str, bytes: &[u8]) {
        let Some(f) = self.file(kind, path) else {
            return;
        };
        if bytes.len() > MAX_BASE_BYTES {
            let _ = std::fs::remove_file(&f);
            return;
        }
        if let Some(d) = f.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        let tmp = f.with_extension("md.partial");
        if std::fs::write(&tmp, bytes).is_ok() {
            let _ = std::fs::rename(&tmp, &f);
        }
    }

    fn load(&self, kind: &str, path: &str) -> Option<Vec<u8>> {
        std::fs::read(self.file(kind, path)?).ok()
    }

    fn remove(&self, kind: &str, path: &str) {
        if let Some(f) = self.file(kind, path) {
            let _ = std::fs::remove_file(f);
        }
    }

    /// Remembers the content of `path` as synced.
    pub fn save_synced(&self, path: &str, bytes: &[u8]) {
        self.save("synced", path, bytes);
    }

    /// A copy of `path` as synced is kept.
    pub fn has_synced(&self, path: &str) -> bool {
        self.file("synced", path).is_some_and(|f| f.is_file())
    }

    /// The content of `path` at the last sync.
    pub fn synced(&self, path: &str) -> Option<Vec<u8>> {
        self.load("synced", path)
    }

    /// Forgets the synced content of `path`.
    pub fn remove_synced(&self, path: &str) {
        self.remove("synced", path);
    }

    /// Remembers the common ancestor of the conflict copy `copy`.
    pub fn save_ancestor(&self, copy: &str, bytes: &[u8]) {
        self.save("ancestor", copy, bytes);
    }

    /// The common ancestor of the conflict copy `copy`.
    pub fn ancestor(&self, copy: &str) -> Option<Vec<u8>> {
        self.load("ancestor", copy)
    }

    /// Forgets the ancestor of `copy` (the copy is resolved).
    pub fn remove_ancestor(&self, copy: &str) {
        self.remove("ancestor", copy);
    }

    /// The folder, for tests.
    pub fn dir(&self) -> Option<&Path> {
        self.dir.as_deref()
    }
}
