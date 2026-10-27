//! A lock that keeps two programs from having the same graph open (MG-1): two apps would each index it, sync it and
//! write to it, and the second sync loop would fight the first. The lock is an advisory file lock (`flock`) that the
//! system drops by itself when the program ends, so a crash leaves nothing behind.
//!
//! The lock file is not in the graph folder (it would be synced and backed up) and not in the device's data folder
//! (the installed app and the dev app have different ones, and both can open the same graph folder): it is in the
//! system's temp folder, named after the graph folder.

use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, Write};
use std::path::{Path, PathBuf};

/// Holds the lock of a graph folder until it is dropped.
pub struct GraphLock {
    file: File,
    root: PathBuf,
}

/// Why a graph could not be locked.
#[derive(Debug)]
pub enum LockError {
    /// Another program has the graph open: what its lock file says about it (its process number).
    Held(String),
    /// The lock file could not be made.
    Io(std::io::Error),
}

impl std::fmt::Display for LockError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LockError::Held(who) if who == STILL_CLOSING => {
                f.write_str("this graph is still being closed: try again in a moment")
            }
            LockError::Held(who) => write!(
                f,
                "this graph is open in another Notatnik{}: close it there first",
                if who.is_empty() {
                    String::new()
                } else {
                    format!(" ({who})")
                }
            ),
            LockError::Io(e) => write!(f, "the lock of the graph could not be made: {e}"),
        }
    }
}

impl std::error::Error for LockError {}

/// What `Held` says when the lock is this very program's: a graph that was closed a moment ago still finishes its last
/// sync and snapshot before it lets go.
const STILL_CLOSING: &str = "this program";

fn lock_dir() -> PathBuf {
    let user = std::env::var("USER")
        .or_else(|_| std::env::var("LOGNAME"))
        .unwrap_or_else(|_| "user".into());
    let user: String = user
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
        .collect();
    std::env::temp_dir().join(format!("notatnik-locks-{user}"))
}

impl GraphLock {
    /// Locks the graph folder `root`. Fails with [`LockError::Held`] when another program (or another `GraphLock` of this
    /// one) has it.
    pub fn acquire(root: &Path) -> Result<GraphLock, LockError> {
        let root = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
        let dir = lock_dir();
        std::fs::create_dir_all(&dir).map_err(LockError::Io)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700));
        }
        let name = blake3::hash(root.to_string_lossy().as_bytes()).to_hex();
        let path = dir.join(format!("{}.lock", &name[..16]));
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .map_err(LockError::Io)?;
        match file.try_lock() {
            Ok(()) => {}
            Err(std::fs::TryLockError::WouldBlock) => {
                let mut who = String::new();
                let _ = file.read_to_string(&mut who);
                let who = who.lines().next().unwrap_or("").to_string();
                let mine = format!("process {}", std::process::id());
                return Err(LockError::Held(if who == mine {
                    STILL_CLOSING.to_string()
                } else {
                    who
                }));
            }
            Err(std::fs::TryLockError::Error(e)) => return Err(LockError::Io(e)),
        }
        // say who holds it, for the message of the next one
        let _ = file.set_len(0);
        let _ = file.rewind();
        let _ = writeln!(file, "process {}", std::process::id());
        Ok(GraphLock { file, root })
    }

    /// The (canonical) graph folder this lock is for.
    pub fn root(&self) -> &Path {
        &self.root
    }
}

impl Drop for GraphLock {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_folder_is_locked_once_and_free_again_when_the_lock_is_dropped() {
        let dir = tempfile::tempdir().unwrap();
        let a = GraphLock::acquire(dir.path()).unwrap();
        let err = GraphLock::acquire(dir.path()).err().expect("locked");
        assert!(
            matches!(&err, LockError::Held(who) if who == STILL_CLOSING),
            "the holder is this very process: {err}"
        );
        assert!(err.to_string().contains("still being closed"));
        // another folder is another lock
        let other = tempfile::tempdir().unwrap();
        let _b = GraphLock::acquire(other.path()).unwrap();
        drop(a);
        assert!(GraphLock::acquire(dir.path()).is_ok());
    }
}
