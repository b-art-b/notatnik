//! Servers to link files from: the list in the device settings, and browsing a folder of one. Only WebDAV for now;
//! the picker asks for a source ID and a folder, so another kind of server can be added behind the same commands.

use notatnik_graph::FileSource;
use notatnik_sync::{Entry, Remote, WebDavRemote, folder_url, item_url, web_base};

use super::syncing::{api_err, secret_key};
use super::*;

/// The source that is the open graph's own server.
const GRAPH_SOURCE: &str = "graph";

fn password_key(id: &str) -> String {
    format!("file source {id}")
}

/// The account's files folder of a Nextcloud sync address (`…/remote.php/dav/files/me`), so the picker starts at
/// the top of the account and not in the graph's folder; any other address is used as it is.
fn account_root(url: &str) -> String {
    match url.split_once("/remote.php/dav/files/") {
        Some((base, rest)) => {
            let user = rest.split('/').next().unwrap_or("");
            format!("{base}/remote.php/dav/files/{user}")
        }
        None => url.trim_end_matches('/').to_string(),
    }
}

/// What it takes to talk to a source.
struct Login {
    server: String,
    user: String,
    password: String,
}

impl AppState {
    /// The server of the open graph's sync as a source, if it has one.
    fn graph_source(&self) -> Option<FileSourceView> {
        let id = self.with_open(|o| Ok(o.graph.id())).ok()?;
        let cfg = self.remote_of(&id)?;
        Some(FileSourceView {
            id: GRAPH_SOURCE.into(),
            name: "This graph's server".into(),
            server: account_root(&cfg.url),
            user: cfg.user,
            graph: true,
        })
    }

    fn login(&self, source: &str) -> ApiResult<Login> {
        let missing_password = || {
            ApiError::new(
                "auth",
                "no password is stored for this server; edit the source in Settings and enter it again",
            )
        };
        if source == GRAPH_SOURCE {
            let id = self.with_open(|o| Ok(o.graph.id()))?;
            let cfg = self.remote_of(&id).ok_or_else(|| {
                ApiError::new("not_found", "this graph is not synced with a server")
            })?;
            let password = self
                .secrets
                .get(&secret_key(&cfg))
                .ok_or_else(missing_password)?;
            return Ok(Login {
                server: account_root(&cfg.url),
                user: cfg.user,
                password,
            });
        }
        let dev = notatnik_graph::DeviceSettings::load(&self.dirs).unwrap_or_default();
        let s = dev
            .file_sources
            .into_iter()
            .find(|s| s.id == source)
            .ok_or_else(|| ApiError::new("not_found", "that server is not in the list"))?;
        let password = self
            .secrets
            .get(&password_key(&s.id))
            .ok_or_else(missing_password)?;
        Ok(Login {
            server: s.server,
            user: s.user,
            password,
        })
    }

    /// `file_sources`: the servers files can be linked from, the open graph's own first.
    pub fn file_sources(&self, _: NoArgs) -> ApiResult<Vec<FileSourceView>> {
        let dev = notatnik_graph::DeviceSettings::load(&self.dirs).unwrap_or_default();
        let mut out: Vec<FileSourceView> = self.graph_source().into_iter().collect();
        out.extend(dev.file_sources.into_iter().map(|s| FileSourceView {
            id: s.id,
            name: s.name,
            server: s.server,
            user: s.user,
            graph: false,
        }));
        Ok(out)
    }

    /// `file_source_save`: adds a server or changes one, after logging in to it once.
    pub fn file_source_save(&self, a: FileSourceArgs) -> ApiResult<FileSourceView> {
        let mut dev = notatnik_graph::DeviceSettings::load(&self.dirs).unwrap_or_default();
        let existing =
            a.id.as_deref()
                .and_then(|id| dev.file_sources.iter().find(|s| s.id == id))
                .cloned();
        if a.id.is_some() && existing.is_none() {
            return Err(ApiError::new("not_found", "that server is not in the list"));
        }
        let name = a.name.trim();
        if name.is_empty() {
            return Err(ApiError::new("invalid", "give the server a name"));
        }
        let id = existing
            .as_ref()
            .map_or_else(notatnik_graph::new_graph_id, |s| s.id.clone());
        let password = if a.password.is_empty() {
            self.secrets
                .get(&password_key(&id))
                .ok_or_else(|| ApiError::new("invalid", "enter the password"))?
        } else {
            a.password.clone()
        };
        // log in once: a wrong address or password is told now, not when the picker opens
        let url = folder_url(&a.server, &a.user, "").map_err(|e| api_err(&e))?;
        let remote = WebDavRemote::new(&url, a.user.trim(), &password).map_err(|e| api_err(&e))?;
        if remote.list("").map_err(|e| api_err(&e))?.is_none() {
            return Err(ApiError::new(
                "not_found",
                "the server has no files folder at that address",
            ));
        }
        if !a.password.is_empty() {
            self.secrets
                .set(&password_key(&id), &password)
                .map_err(|e| {
                    ApiError::new("server", format!("the password could not be stored: {e}"))
                })?;
        }
        let entry = FileSource {
            id: id.clone(),
            name: name.to_string(),
            server: a.server.trim().to_string(),
            user: a.user.trim().to_string(),
        };
        match dev.file_sources.iter_mut().find(|s| s.id == id) {
            Some(slot) => *slot = entry.clone(),
            None => dev.file_sources.push(entry.clone()),
        }
        dev.save(&self.dirs)?;
        Ok(FileSourceView {
            id,
            name: entry.name,
            server: entry.server,
            user: entry.user,
            graph: false,
        })
    }

    /// `file_source_delete`: removes a server and forgets its password; the list that is left.
    pub fn file_source_delete(&self, a: FileSourceIdArgs) -> ApiResult<Vec<FileSourceView>> {
        let mut dev = notatnik_graph::DeviceSettings::load(&self.dirs).unwrap_or_default();
        dev.file_sources.retain(|s| s.id != a.id);
        dev.save(&self.dirs)?;
        self.secrets.delete(&password_key(&a.id));
        self.file_sources(NoArgs {})
    }

    /// `file_browse`: the folders and files directly in a folder of a source, with a link for each file.
    pub fn file_browse(&self, a: FileBrowseArgs) -> ApiResult<FileBrowse> {
        let login = self.login(&a.source)?;
        let mut parts = Vec::new();
        for seg in a.path.split('/').map(str::trim).filter(|s| !s.is_empty()) {
            if seg == "." || seg == ".." {
                return Err(ApiError::new(
                    "invalid",
                    "the folder cannot contain `.` or `..`",
                ));
            }
            parts.push(seg);
        }
        let path = parts.join("/");
        let root = folder_url(&login.server, &login.user, "").map_err(|e| api_err(&e))?;
        let remote =
            WebDavRemote::new(&root, &login.user, &login.password).map_err(|e| api_err(&e))?;
        let listing = remote.list(&path).map_err(|e| api_err(&e))?;
        let exists = listing.is_some();
        let entries = listing.map(|l| l.entries).unwrap_or_default();
        let web = web_base(&root);
        let link_of = |e: &Entry| {
            let id = Some(&e.file_id).filter(|i| !i.is_empty());
            match (id, &web) {
                (Some(id), Some(web)) => (format!("{web}/f/{id}"), false),
                _ => (item_url(&root, &format!("{path}/{}", e.name)), true),
            }
        };
        let mut folders: Vec<FolderItem> = entries
            .iter()
            .filter(|e| e.dir && !e.name.starts_with('.'))
            .map(|e| {
                let (link, by_path) = link_of(e);
                FolderItem {
                    name: e.name.clone(),
                    link,
                    by_path,
                }
            })
            .collect();
        folders.sort_by_key(|f| f.name.to_lowercase());
        let mut files: Vec<FileItem> = entries
            .iter()
            .filter(|e| !e.dir && !e.name.starts_with('.'))
            .map(|e| {
                let (link, by_path) = link_of(e);
                FileItem {
                    name: e.name.clone(),
                    size: e.size as f64,
                    link,
                    by_path,
                }
            })
            .collect();
        files.sort_by_key(|f| f.name.to_lowercase());
        let root_item = match &web {
            Some(web) => FolderItem {
                name: "All files".into(),
                link: format!("{web}/apps/files/"),
                by_path: false,
            },
            None => FolderItem {
                name: "All files".into(),
                link: item_url(&root, ""),
                by_path: true,
            },
        };
        Ok(FileBrowse {
            root: root_item,
            path,
            exists,
            folders,
            files,
        })
    }
}
