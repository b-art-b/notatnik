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
    /// The server copy is stored encrypted (`<path>.enc`, EN-12).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub enc: bool,
    /// The server copy is a hidden object (EN-25): its name on the server. The path is inside the object, so this
    /// is how the device finds its way from a name to a path without opening the object again.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
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

    /// Makes every file that the server holds encrypted look changed here, so that the next sync stores it again with
    /// the key that is current then, without reading the old copy (EN-22: the key was replaced, the old copies cannot
    /// be opened any more). The server's ETags are kept, so a copy that someone else changed meanwhile is still a
    /// conflict, not overwritten. Returns how many files.
    pub fn store_encrypted_again(&mut self) -> usize {
        let mut n = 0;
        for f in self.files.values_mut().filter(|f| f.enc) {
            f.hash = "store-again".into();
            f.mtime_ms = -1; // the scan reads the file again instead of trusting this hash
            n += 1;
        }
        n
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
