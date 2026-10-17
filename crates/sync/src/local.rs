//! The replica folder: what is in it, and atomic writes into it.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::UNIX_EPOCH;

use crate::Result;
use crate::state::SyncState;

/// A file of the replica.
#[derive(Debug, Clone)]
pub struct LocalFile {
    pub hash: String,
    pub size: u64,
    pub mtime_ms: i64,
}

/// Paths that are never synced: temp files of atomic writes, the local trash, editor leftovers.
pub fn ignored(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    path.starts_with(".notatnik/tmp/")
        || path.starts_with(".notatnik/trash/")
        || name.ends_with(".partial")
        || name.ends_with('~')
        || name == ".DS_Store"
        || name == "Thumbs.db"
}

pub fn hash(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}

pub fn mtime_ms(md: &std::fs::Metadata) -> i64 {
    md.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_millis() as i64)
}

/// Every syncable file below `root`, with its hash. A file whose size and time are those of the last sync keeps
/// the hash from the state instead of being read again.
pub fn scan(root: &Path, state: &SyncState) -> Result<BTreeMap<String, LocalFile>> {
    let mut out = BTreeMap::new();
    walk(root, "", state, &mut out)?;
    Ok(out)
}

fn walk(
    dir: &Path,
    rel: &str,
    state: &SyncState,
    out: &mut BTreeMap<String, LocalFile>,
) -> Result<()> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Ok(());
    };
    for e in rd {
        let e = e?;
        let Some(name) = e.file_name().to_str().map(String::from) else {
            continue; // names that are not UTF-8 cannot be sent
        };
        let path = if rel.is_empty() {
            name
        } else {
            format!("{rel}/{name}")
        };
        let ft = e.file_type()?;
        if ft.is_symlink() {
            continue;
        }
        if ft.is_dir() {
            if !ignored(&format!("{path}/")) {
                walk(&e.path(), &path, state, out)?;
            }
        } else if ft.is_file() && !ignored(&path) {
            let md = e.metadata()?;
            let (size, mtime) = (md.len(), mtime_ms(&md));
            let known = state
                .files
                .get(&path)
                .filter(|s| s.size == size && s.mtime_ms == mtime);
            let hash = match known {
                Some(s) => s.hash.clone(),
                None => match std::fs::read(e.path()) {
                    Ok(b) => hash(&b),
                    Err(_) => continue, // vanished meanwhile
                },
            };
            out.insert(
                path,
                LocalFile {
                    hash,
                    size,
                    mtime_ms: mtime,
                },
            );
        }
    }
    Ok(())
}

/// Writes `bytes` to `root/rel` atomically (temp file in `.notatnik/tmp/`, then rename) and returns the
/// modification time of the new file.
pub fn write_atomic(root: &Path, rel: &str, bytes: &[u8]) -> Result<i64> {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let tmp_dir = root.join(".notatnik").join("tmp");
    std::fs::create_dir_all(&tmp_dir)?;
    let tmp = tmp_dir.join(format!(
        "sync{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let target = root.join(rel);
    let result = (|| -> Result<i64> {
        std::fs::write(&tmp, bytes)?;
        if let Some(p) = target.parent() {
            std::fs::create_dir_all(p)?;
        }
        std::fs::rename(&tmp, &target)?;
        Ok(mtime_ms(&std::fs::metadata(&target)?))
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

/// Moves a file to `.notatnik/trash/<path>` (with a number when that name is taken) instead of deleting it.
pub fn to_trash(root: &Path, rel: &str) -> Result<PathBuf> {
    let base = root.join(".notatnik").join("trash").join(rel);
    let mut target = base.clone();
    let mut n = 1;
    while target.exists() {
        target = PathBuf::from(format!("{}.{n}", base.display()));
        n += 1;
    }
    if let Some(p) = target.parent() {
        std::fs::create_dir_all(p)?;
    }
    std::fs::rename(root.join(rel), &target)?;
    Ok(target)
}
