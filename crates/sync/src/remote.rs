use crate::Result;

/// One item of a folder on the server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// The name, one path segment, decoded.
    pub name: String,
    /// A folder.
    pub dir: bool,
    /// The ETag, without quotes (see [`norm_etag`]).
    pub etag: String,
    /// Size in bytes (0 for folders).
    pub size: u64,
}

/// A folder on the server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listing {
    /// The ETag of the folder itself; on Nextcloud it changes whenever anything below changes.
    pub etag: String,
    /// What is directly inside.
    pub entries: Vec<Entry>,
}

/// The condition of a write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cond {
    /// Only if the file does not exist (`If-None-Match: *`).
    Create,
    /// Only if the file still has this ETag (`If-Match`).
    Match(String),
}

/// What the sync engine needs from a server. Paths are relative to the synced folder, with `/` between segments,
/// and `""` is the folder itself.
pub trait Remote: Send + Sync {
    /// The content of a folder; `None` when it does not exist.
    fn list(&self, dir: &str) -> Result<Option<Listing>>;
    /// A file's bytes and its ETag.
    fn get(&self, path: &str) -> Result<(Vec<u8>, String)>;
    /// Writes a file under a condition and returns its new ETag. A failed condition is `Error::Changed`.
    fn put(&self, path: &str, data: &[u8], cond: Cond) -> Result<String>;
    /// Creates a folder and its missing parents; fine if it exists.
    fn mkdir(&self, dir: &str) -> Result<()>;
    /// Deletes a file, if it still has that ETag when one is given. A file that is gone already is fine.
    fn delete(&self, path: &str, if_match: Option<&str>) -> Result<()>;
}

/// An ETag as the engine compares it: without the weak marker and the quotes.
pub fn norm_etag(s: &str) -> String {
    let s = s.trim();
    let s = s.strip_prefix("W/").unwrap_or(s);
    s.trim_matches('"').to_string()
}
