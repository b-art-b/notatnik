//! Documents of the graph's Nextcloud (NA-1 to NA-8, NX1): a link to a PDF or an EPUB in the account of the sync server is
//! resolved to the file, downloaded with the sync login into a cache of this device (never into the graph) and shown by
//! the viewers. The cache is thrown away at will; "keep a copy" turns a document into an ordinary asset.
//!
//! Nothing here is logged above `debug` but the number of the file and its size: not the address, the path or the name.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use notatnik_sync::{Error as SyncError, FileInfo, WebDavRemote, item_url, web_base};
use serde::{Deserialize, Serialize};

use super::filesources::account_root;
use super::syncing::api_err;
use super::*;

/// A document larger than this is not downloaded (NA-5).
pub(super) const MAX_BYTES: u64 = 100 * 1024 * 1024;
/// The cache of all graphs of this device is cleaned above this size, oldest file first.
pub(super) const CACHE_LIMIT: u64 = 500 * 1024 * 1024;
/// The prefix of a cached file as the viewers and the asset route see it.
pub(super) const CACHE_PREFIX: &str = "cache/remote/";
const META: &str = "meta.json";

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Target {
    /// `<server>/f/<id>`
    Id(String),
    /// A path below the account's files.
    Path(String),
}

/// What is known about a cached document, for the time the server is out of reach.
#[derive(Serialize, Deserialize)]
struct Meta {
    name: String,
    address: String,
    etag: String,
    size: u64,
    /// `YYYY-MM-DD`
    fetched: String,
}

fn decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && i + 2 < b.len()
            && let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16)
        {
            out.push(v);
            i += 3;
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// What a link to the server means: `web_base` is the server as the browser knows it, `user` the account of the sync
/// login. `None` for a link to anything else (another host, a page of the web interface, another account).
pub(super) fn parse_link(link: &str, web_base: &str, user: &str) -> Option<Target> {
    let link = link.trim();
    let base = web_base.trim_end_matches('/');
    // scheme and host are not case sensitive, the path is
    if link.len() <= base.len() + 1
        || !link.is_char_boundary(base.len())
        || !link[..base.len()].eq_ignore_ascii_case(base)
        || link.as_bytes()[base.len()] != b'/'
    {
        return None;
    }
    let rest = &link[base.len() + 1..];
    let rest = rest.split(['?', '#']).next().unwrap_or("");
    if let Some(id) = rest.strip_prefix("f/") {
        let id = id.trim_end_matches('/');
        return (!id.is_empty() && id.bytes().all(|b| b.is_ascii_digit()))
            .then(|| Target::Id(id.to_string()));
    }
    let tail = rest.strip_prefix("remote.php/dav/files/")?;
    let (who, path) = tail.split_once('/')?;
    if decode(who) != user {
        return None;
    }
    let path = decode(path);
    let path = path.trim_matches('/');
    if path.is_empty()
        || path
            .split('/')
            .any(|s| s.is_empty() || s == "." || s == "..")
        || path.contains('\0')
        || path.contains('\\')
    {
        return None;
    }
    Some(Target::Path(path.to_string()))
}

fn kind_of(name: &str) -> Option<&'static str> {
    let lower = name.to_ascii_lowercase();
    if lower.ends_with(".pdf") {
        Some("pdf")
    } else if lower.ends_with(".epub") {
        Some("epub")
    } else {
        None
    }
}

fn path_key(path: &str) -> String {
    format!("p-{}", &blake3::hash(path.as_bytes()).to_hex()[..16])
}

fn safe_etag(etag: &str) -> String {
    let s: String = etag
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .take(64)
        .collect();
    if s.is_empty() { "noetag".into() } else { s }
}

/// An error of the server in words that do not carry the path of the file (the log and the toast show them).
fn remote_err(e: &SyncError) -> ApiError {
    match e {
        SyncError::Auth(_) => ApiError::new(
            "auth",
            "The server refused the login; check the password in Settings → Sync",
        ),
        SyncError::NotFound(_) => ApiError::new(
            "not_found",
            "The file is not on the server (it may have been deleted, or the server cannot look files up by number)",
        ),
        SyncError::Offline(_) => ApiError::new("offline", "The server cannot be reached"),
        SyncError::TooLarge(n) => ApiError::new(
            "too_large",
            format!(
                "The file is too large to open here ({} MB, the limit is 100 MB); open it in Nextcloud",
                n / 1_000_000
            ),
        ),
        SyncError::Server(m) => {
            // `HTTP 500 for Docs/Report.pdf`: the part after `for` is the path
            let m = m.split(" for ").next().unwrap_or(m);
            ApiError::new("server", format!("The server could not give the file: {m}"))
        }
        _ => ApiError::new("server", "The server could not give the file"),
    }
}

fn not_opened(reason: &str, name: &str) -> RemoteOpened {
    RemoteOpened {
        opened: false,
        reason: reason.into(),
        kind: String::new(),
        name: name.into(),
        address: String::new(),
        path: String::new(),
        size: 0,
        etag: String::new(),
        offline: false,
        fetched: String::new(),
        downloaded: false,
    }
}

impl AppState {
    /// Lets the cache of downloaded documents grow only to `bytes`. For tests; the app keeps 500 MB.
    pub fn with_remote_cache_limit(mut self, bytes: u64) -> Self {
        self.remote_cache_limit = bytes;
        self
    }

    /// The folder of the downloaded documents of the open graph.
    fn remote_dir(&self) -> ApiResult<PathBuf> {
        let id = self.with_open(|o| Ok(o.graph.id()))?;
        Ok(self.dirs.graph_data(&id).join("remote"))
    }

    /// A file of the cache named the way the viewers name it (`cache/remote/<key>/<etag>.<ext>`), checked to be inside
    /// the cache of the open graph, whatever symbolic links there are (NA-8).
    pub(super) fn cache_file(&self, rel: &str) -> ApiResult<PathBuf> {
        let bad = || ApiError::new("invalid", "that is not a downloaded document");
        let clean = rel.trim_start_matches("../").trim_start_matches("./");
        let rest = clean.strip_prefix(CACHE_PREFIX).ok_or_else(bad)?;
        if rest.contains('\\')
            || rest.contains('\0')
            || rest
                .split('/')
                .any(|s| s.is_empty() || s == "." || s == "..")
        {
            return Err(bad());
        }
        let base = self.remote_dir()?;
        let not_found = || ApiError::new("not_found", "asset not found");
        let base = base.canonicalize().map_err(|_| not_found())?;
        let full = base.join(rest).canonicalize().map_err(|_| not_found())?;
        if !full.starts_with(&base) || !full.is_file() {
            return Err(bad());
        }
        Ok(full)
    }

    /// `remote_open`: resolves a link to a document of the graph's server, fetches it into the cache unless the copy is
    /// current, and tells where it is. A link that is not a PDF or an EPUB of this server answers `opened: false`.
    pub fn remote_open(&self, a: RemoteOpenArgs) -> ApiResult<RemoteOpened> {
        let Some(src) = self.graph_source() else {
            return Ok(not_opened("not_synced", ""));
        };
        let Some(base) = web_base(&src.server) else {
            return Ok(not_opened("other_server", ""));
        };
        let Some(target) = parse_link(&a.url, &base, &src.user) else {
            return Ok(not_opened("other_server", ""));
        };
        let login = self.login("graph")?;
        let remote = WebDavRemote::for_documents(&login.server, &login.user, &login.password)
            .map_err(|e| api_err(&e))?;
        let found = match &target {
            Target::Id(id) => remote.find_by_id(id),
            Target::Path(p) => remote.stat(p),
        };
        let info = match found {
            Ok(Some(i)) => i,
            Ok(None) => return Err(remote_err(&SyncError::NotFound(String::new()))),
            Err(SyncError::Offline(_)) => return self.remote_from_cache(&target),
            Err(e) => return Err(remote_err(&e)),
        };
        let Some(kind) = kind_of(&info.name) else {
            return Ok(not_opened("other_type", &info.name));
        };
        if info.size > MAX_BYTES {
            return Err(remote_err(&SyncError::TooLarge(info.size)));
        }
        let key = if info.file_id.is_empty() {
            path_key(&info.path)
        } else {
            info.file_id.clone()
        };
        let address = if info.file_id.is_empty() {
            item_url(&account_root(&src.server), &info.path)
        } else {
            format!("{base}/f/{}", info.file_id)
        };
        let dir = self.remote_dir()?.join(&key);
        let ext = if kind == "pdf" { "pdf" } else { "epub" };
        let etag = safe_etag(&info.etag);
        let file = dir.join(format!("{etag}.{ext}"));
        // a server without ETags cannot say that the copy is current: fetch again
        let current = file.is_file() && !info.etag.is_empty();
        if !current {
            std::fs::create_dir_all(&dir).map_err(|e| ApiError::new("server", e.to_string()))?;
            remote
                .download(&info.path, MAX_BYTES, &file)
                .map_err(|e| remote_err(&e))?;
            self.tidy_dir(&dir, &file);
        }
        touch(&file);
        let meta = Meta {
            name: info.name.clone(),
            address: address.clone(),
            etag: info.etag.clone(),
            size: info.size,
            fetched: self.today().to_string(),
        };
        if !current {
            write_meta(&dir, &meta);
        }
        if info.file_id.is_empty() {
            self.remember_path_key(&info);
        }
        self.evict(&file);
        log::info!(
            "opened a Nextcloud document ({key}, {} bytes, {})",
            info.size,
            if current { "from the cache" } else { "fetched" }
        );
        let fetched = if current {
            read_meta(&dir).map_or(meta.fetched.clone(), |m| m.fetched)
        } else {
            meta.fetched.clone()
        };
        Ok(RemoteOpened {
            opened: true,
            reason: String::new(),
            kind: kind.into(),
            name: info.name,
            address,
            path: format!("{CACHE_PREFIX}{key}/{etag}.{ext}"),
            size: info.size,
            etag: info.etag,
            offline: false,
            fetched,
            downloaded: !current,
        })
    }

    /// The server is out of reach: the copy of an earlier open, if there is one.
    fn remote_from_cache(&self, target: &Target) -> ApiResult<RemoteOpened> {
        let offline = || {
            ApiError::new(
                "offline",
                "The server cannot be reached, and this document has not been opened on this device before",
            )
        };
        let root = self.remote_dir()?;
        let key = match target {
            Target::Id(id) => id.clone(),
            Target::Path(p) => std::fs::read_to_string(root.join("paths").join(path_key(p)))
                .map_err(|_| offline())?
                .trim()
                .to_string(),
        };
        if key.is_empty() || key.contains(['/', '\\']) || key.contains("..") {
            return Err(offline());
        }
        let dir = root.join(&key);
        let meta = read_meta(&dir).ok_or_else(offline)?;
        let file = newest_file(&dir).ok_or_else(offline)?;
        let name = file
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        let kind = kind_of(&meta.name).ok_or_else(offline)?;
        touch(&file);
        log::info!(
            "opened a Nextcloud document from the cache, the server is out of reach ({key})"
        );
        Ok(RemoteOpened {
            opened: true,
            reason: String::new(),
            kind: kind.into(),
            name: meta.name,
            address: meta.address,
            path: format!("{CACHE_PREFIX}{key}/{name}"),
            size: meta.size,
            etag: meta.etag,
            offline: true,
            fetched: meta.fetched,
            downloaded: false,
        })
    }

    /// A server without file numbers: the cache folder is named by the path, and this remembers which.
    fn remember_path_key(&self, info: &FileInfo) {
        let Ok(root) = self.remote_dir() else { return };
        let dir = root.join("paths");
        if std::fs::create_dir_all(&dir).is_ok() {
            let _ = std::fs::write(dir.join(path_key(&info.path)), path_key(&info.path));
        }
    }

    /// Removes the older copies of a document (every file of the folder but `keep` and the notes about it).
    fn tidy_dir(&self, dir: &Path, keep: &Path) {
        let Ok(rd) = std::fs::read_dir(dir) else {
            return;
        };
        for e in rd.flatten() {
            let p = e.path();
            if p != keep && e.file_name() != META {
                let _ = std::fs::remove_file(p);
            }
        }
    }

    /// Keeps the cache of this device under the limit by deleting the least recently opened files, never `keep`.
    fn evict(&self, keep: &Path) {
        let mut files: Vec<(SystemTime, u64, PathBuf)> = Vec::new();
        let graphs = self.dirs.data.join("graphs");
        let Ok(rd) = std::fs::read_dir(&graphs) else {
            return;
        };
        for g in rd.flatten() {
            let Ok(keys) = std::fs::read_dir(g.path().join("remote")) else {
                continue;
            };
            for k in keys.flatten() {
                let Ok(docs) = std::fs::read_dir(k.path()) else {
                    continue;
                };
                for d in docs.flatten() {
                    if d.file_name() == META {
                        continue;
                    }
                    if let Ok(m) = d.metadata()
                        && m.is_file()
                    {
                        files.push((
                            m.modified().unwrap_or(SystemTime::UNIX_EPOCH),
                            m.len(),
                            d.path(),
                        ));
                    }
                }
            }
        }
        let mut total: u64 = files.iter().map(|f| f.1).sum();
        if total <= self.remote_cache_limit {
            return;
        }
        files.sort();
        for (_, size, path) in files {
            if total <= self.remote_cache_limit {
                break;
            }
            if path == keep {
                continue;
            }
            if std::fs::remove_file(&path).is_ok() {
                total = total.saturating_sub(size);
                if let Some(dir) = path.parent() {
                    // a folder with only its notes left goes too
                    if newest_file(dir).is_none() {
                        let _ = std::fs::remove_dir_all(dir);
                    }
                }
            }
        }
    }

    /// `remote_cache_clear`: deletes every downloaded document of this device.
    pub fn remote_cache_clear(&self, _a: NoArgs) -> ApiResult<CacheCleared> {
        let mut freed = 0;
        if let Ok(rd) = std::fs::read_dir(self.dirs.data.join("graphs")) {
            for g in rd.flatten() {
                let dir = g.path().join("remote");
                freed += dir_size(&dir);
                let _ = std::fs::remove_dir_all(&dir);
            }
        }
        Ok(CacheCleared { freed })
    }

    /// `keep_document`: copies a downloaded document into the graph's `assets/` (up to 100 MB) and points the sidecar of the
    /// document, if it has one, at the copy. Links in notes are not rewritten; the content hash still finds the sidecar.
    pub fn keep_document(&self, a: KeepDocumentArgs) -> ApiResult<KeptDocument> {
        let (file, name) = self.document_file(&a.path)?;
        let page = Self::parse_name(&a.page)?;
        let bytes = std::fs::read(&file).map_err(|e| ApiError::new("not_found", e.to_string()))?;
        let sha = notatnik_graph::sha_of_file(&file).map_err(ApiError::from)?;
        let today = self.today();
        self.with_open(|o| {
            let path = o
                .graph
                .save_attachment_up_to(&bytes, &name, today, MAX_BYTES as usize)?;
            let page_rel = Self::page_file(o, &page)?;
            let link = notatnik_graph::relative_asset_path(&page_rel, &path);
            let label: String = name
                .chars()
                .flat_map(|c| match c {
                    '[' | ']' | '\\' => vec!['\\', c],
                    '\n' | '\r' => vec![' '],
                    c => vec![c],
                })
                .collect();
            let (by_file, by_sha, kinds) = o.graph.with_index(|i| {
                Ok::<_, notatnik_index::Error>((
                    i.page_property_values("file")?,
                    i.pages_with_property("sha", &sha)?,
                    i.pages_with_property("kind", ANNOTATIONS_KIND)?,
                ))
            })?;
            let is_sidecar = |p: &str| kinds.iter().any(|k| k == p);
            let sidecar = by_file
                .iter()
                .find(|(p, v)| *v == a.address && is_sidecar(p))
                .map(|(p, _)| p.clone())
                .or_else(|| by_sha.iter().find(|p| is_sidecar(p)).cloned());
            let mut moved = false;
            if let Some(p) = sidecar {
                let name = Self::parse_name(&p)?;
                let rel = Self::page_file(o, &name)?;
                let hash = o.graph.read_page(&rel)?.hash;
                o.graph.edit_page(
                    &rel,
                    Some(&hash),
                    &[BlockOp::SetPageProperty {
                        key: "file".into(),
                        value: yaml_item(&path),
                    }],
                )?;
                self.emit_page_changed(&p);
                moved = true;
            }
            Ok(KeptDocument {
                markdown: format!("[{label}]({link})"),
                path,
                sidecar_moved: moved,
            })
        })
    }
}

fn read_meta(dir: &Path) -> Option<Meta> {
    serde_json::from_slice(&std::fs::read(dir.join(META)).ok()?).ok()
}

fn write_meta(dir: &Path, meta: &Meta) {
    if let Ok(json) = serde_json::to_vec(meta) {
        let _ = std::fs::write(dir.join(META), json);
    }
}

/// The most recently opened document file of a cache folder (not the notes about it).
fn newest_file(dir: &Path) -> Option<PathBuf> {
    std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .filter(|e| e.file_name() != META && e.path().is_file())
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .max()
        .map(|(_, p)| p)
}

/// Marks a cached file as opened now (the cache is cleaned oldest first).
fn touch(file: &Path) {
    if let Ok(f) = std::fs::OpenOptions::new().write(true).open(file) {
        let _ = f.set_modified(SystemTime::now());
    }
}

fn dir_size(dir: &Path) -> u64 {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return 0;
    };
    rd.flatten()
        .map(|e| match e.metadata() {
            Ok(m) if m.is_dir() => dir_size(&e.path()),
            Ok(m) => m.len(),
            Err(_) => 0,
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: &str = "https://cloud.example.org";

    #[test]
    fn links_of_the_server_are_told_from_all_others() {
        let t = |l: &str| parse_link(l, BASE, "me");
        assert_eq!(
            t("https://cloud.example.org/f/12345"),
            Some(Target::Id("12345".into()))
        );
        assert_eq!(
            t("HTTPS://Cloud.Example.org/f/7?x=1#y"),
            Some(Target::Id("7".into()))
        );
        assert_eq!(
            t("https://cloud.example.org/remote.php/dav/files/me/Docs/My%20Report.pdf"),
            Some(Target::Path("Docs/My Report.pdf".into()))
        );
        // another account, another host, another page of the web interface
        assert_eq!(
            t("https://cloud.example.org/remote.php/dav/files/you/a.pdf"),
            None
        );
        assert_eq!(t("https://evil.example.org/f/12345"), None);
        assert_eq!(t("https://cloud.example.org.evil.org/f/12345"), None);
        assert_eq!(t("https://cloud.example.org/apps/files/?dir=/x"), None);
        assert_eq!(t("https://cloud.example.org/f/abc"), None);
        assert_eq!(t("https://cloud.example.org/f/"), None);
        assert_eq!(t("https://cloud.example.org"), None);
        // no way out of the account's folder
        assert_eq!(
            t("https://cloud.example.org/remote.php/dav/files/me/../you/a.pdf"),
            None
        );
        assert_eq!(
            t("https://cloud.example.org/remote.php/dav/files/me/a/%2e%2e/b.pdf"),
            None
        );
    }

    #[test]
    fn kinds_and_names() {
        assert_eq!(kind_of("A.PDF"), Some("pdf"));
        assert_eq!(kind_of("book.epub"), Some("epub"));
        assert_eq!(kind_of("a.docx"), None);
        assert_eq!(safe_etag("\"ab/../12\""), "ab12");
        assert_eq!(safe_etag(""), "noetag");
        assert!(path_key("Docs/a.pdf").starts_with("p-"));
    }
}
