use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::Result;

/// What is known about a file at the last sync.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileState {
    /// blake3 of the content, hex.
    pub hash: String,
    /// The ETag the server had for exactly that content.
    pub etag: String,
    /// Size in bytes.
    pub size: u64,
    /// Modification time of the local file after the sync, in milliseconds; with the size it lets the next scan
    /// skip hashing a file that did not change.
    pub mtime_ms: i64,
}

/// The sync state of one replica, kept outside the graph folder (ST-2).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncState {
    /// Per file path.
    pub files: BTreeMap<String, FileState>,
    /// ETags of the remote folders at the last complete sync; used only when the server's folder ETags are trusted.
    pub dirs: BTreeMap<String, String>,
    /// When the last complete sync ended (RFC 3339).
    pub last_sync: Option<String>,
}

impl SyncState {
    /// Loads the state; a missing or unreadable file is an empty state (the next sync compares everything).
    pub fn load(path: &Path) -> SyncState {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    /// Saves the state atomically.
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(p) = path.parent() {
            std::fs::create_dir_all(p)?;
        }
        let tmp = path.with_extension("json.partial");
        std::fs::write(&tmp, serde_json::to_vec(self).unwrap_or_default())?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }
}
