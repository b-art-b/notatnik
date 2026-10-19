//! Sync of a graph with a WebDAV folder (SY-1..SY-8): the credentials store, the background worker, and the
//! commands. The engine itself is in `notatnik-sync`; it works on the graph's folder and never touches `Open`.

use std::collections::HashMap;
use std::sync::Condvar;
use std::time::Instant;

use notatnik_graph::RemoteConfig;
use notatnik_sync::{Error as SyncError, Options, WebDavRemote};

use super::*;

/// Where passwords are kept. The desktop app uses the system's keyring; tests and the dev server keep them in memory.
pub trait Secrets: Send + Sync {
    /// The secret stored under `key`.
    fn get(&self, key: &str) -> Option<String>;
    /// Stores a secret.
    fn set(&self, key: &str, value: &str) -> Result<(), String>;
    /// Removes a secret (fine if there is none).
    fn delete(&self, key: &str);
}

/// Secrets that live until the process ends.
#[derive(Default)]
pub struct MemorySecrets(Mutex<HashMap<String, String>>);

impl Secrets for MemorySecrets {
    fn get(&self, key: &str) -> Option<String> {
        self.0.lock().ok()?.get(key).cloned()
    }
    fn set(&self, key: &str, value: &str) -> Result<(), String> {
        self.0
            .lock()
            .map_err(|e| e.to_string())?
            .insert(key.into(), value.into());
        Ok(())
    }
    fn delete(&self, key: &str) {
        if let Ok(mut m) = self.0.lock() {
            m.remove(key);
        }
    }
}

pub(super) fn secret_key(cfg: &RemoteConfig) -> String {
    format!("webdav {}@{}", cfg.user, cfg.url)
}

pub(super) fn api_err(e: &SyncError) -> ApiError {
    let code = match e {
        SyncError::Auth(_) => "auth",
        SyncError::Offline(_) => "offline",
        SyncError::Url(_) => "invalid",
        SyncError::NotFound(_) => "not_found",
        SyncError::MassDelete(_) => "mass_delete",
        _ => "server",
    };
    ApiError::new(code, e.to_string())
}

/// Where the state of a replica is kept: beside its folder, not in it (ST-2).
fn state_file(replica: &Path) -> PathBuf {
    let name = replica
        .file_name()
        .map_or_else(|| "graph".into(), |n| n.to_string_lossy().into_owned());
    replica.with_file_name(format!("{name}.sync-state.json"))
}

/// Where the pages as last synced are kept, beside the replica and the state file (ST-2).
pub(super) fn bases_dir(replica: &Path) -> PathBuf {
    let name = replica
        .file_name()
        .map_or_else(|| "graph".into(), |n| n.to_string_lossy().into_owned());
    replica.with_file_name(format!("{name}.sync-bases"))
}

/// What the graph's sync is doing and has done, shown in the status line and in the settings.
pub(super) struct SyncCtx {
    root: PathBuf,
    cfg: RemoteConfig,
    secrets: Arc<dyn Secrets>,
    emit: Emitter,
    /// Merge pages changed on both sides (the setting; read at each sync).
    merge: AtomicBool,
    status: Mutex<SyncStatus>,
    /// One sync at a time, whether it was asked for or came from the timer.
    gate: Mutex<()>,
}

/// The sync of the open graph: the shared context and the thread that runs it on a timer.
pub(super) struct SyncTask {
    ctx: Arc<SyncCtx>,
    wake: Arc<Wake>,
}

/// When the worker should sync next.
#[derive(Default)]
pub(super) struct Wake {
    due: Mutex<Option<Instant>>,
    cv: Condvar,
    stop: AtomicBool,
}

impl Wake {
    /// Asks for a sync in `secs` seconds, unless an earlier one is already due.
    pub(super) fn kick(&self, secs: u64) {
        let at = Instant::now() + Duration::from_secs(secs);
        if let Ok(mut d) = self.due.lock() {
            if d.is_none_or(|x| x > at) {
                *d = Some(at);
            }
        }
        self.cv.notify_all();
    }
}

impl Drop for SyncTask {
    fn drop(&mut self) {
        self.wake.stop.store(true, Ordering::Relaxed);
        self.wake.cv.notify_all();
    }
}

const SYNC_EVERY_SECS: u64 = 60;
/// A sync this long after the last change on this device (several edits make one sync).
pub(super) const SYNC_AFTER_WRITE_SECS: u64 = 5;

impl SyncCtx {
    fn set_status(&self, f: impl FnOnce(&mut SyncStatus)) -> SyncStatus {
        let mut s = self
            .status
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        f(&mut s);
        let copy = s.clone();
        drop(s);
        (self.emit)(
            "sync-status",
            serde_json::to_value(&copy).unwrap_or_default(),
        );
        copy
    }

    fn snapshot(&self) -> SyncStatus {
        self.status
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    /// Runs one sync and records how it went.
    fn run(&self, allow_mass_delete: bool) -> SyncStatus {
        let _one_at_a_time = self
            .gate
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let before = self.snapshot();
        self.set_status(|s| s.state = "syncing".into());
        let started = Instant::now();
        let result = self.run_inner(allow_mass_delete);
        self.log_result(&result, &before, started.elapsed());
        self.set_status(|s| match result {
            Ok(report) => {
                s.state = "idle".into();
                s.error_code = None;
                s.error = None;
                s.pending_deletes = 0;
                if report.complete {
                    s.last_sync = Some(chrono::Local::now().to_rfc3339());
                }
                s.pushed = report.pushed.len() as u32;
                s.pulled = report.pulled.len() as u32;
                s.deleted = (report.deleted_remote.len() + report.deleted_local.len()) as u32;
                s.conflicts = report.conflicts.len() as u32;
                s.merged = report.merged.len() as u32;
                // a page another editor joined the lines of stays listed until it looks right again
                let root = self.root.clone();
                let mut warnings: Vec<String> = s
                    .warnings
                    .iter()
                    .filter(|p| {
                        std::fs::read_to_string(root.join(p))
                            .is_ok_and(|t| notatnik_sync::looks_reformatted(&t))
                    })
                    .cloned()
                    .collect();
                for p in &report.suspect {
                    if !warnings.contains(p) {
                        warnings.push(p.clone());
                    }
                }
                s.warnings = warnings;
                s.file_errors = report
                    .errors
                    .iter()
                    .map(|(p, m)| format!("{p}: {m}"))
                    .collect();
                if !s.file_errors.is_empty() {
                    s.state = "error".into();
                    s.error_code = Some("files".into());
                    s.error = Some(format!(
                        "{} file(s) could not be synced; the next sync tries them again",
                        s.file_errors.len()
                    ));
                }
            }
            Err(e) => {
                s.state = if e.code() == "offline" {
                    "offline"
                } else {
                    "error"
                }
                .into();
                s.error_code = Some(api_err(&e).code);
                s.error = Some(e.to_string());
                s.pending_deletes = match e {
                    SyncError::MassDelete(n) => n as u32,
                    _ => 0,
                };
            }
        })
    }

    /// The log line of a sync. Quiet when nothing happened; a problem is reported when it starts and again when it
    /// changes, not at every attempt (a laptop without a network would otherwise log once a minute).
    fn log_result(
        &self,
        result: &Result<notatnik_sync::Report, SyncError>,
        before: &SyncStatus,
        took: Duration,
    ) {
        match result {
            Ok(r) => {
                let done = r.pushed.len()
                    + r.pulled.len()
                    + r.deleted_remote.len()
                    + r.deleted_local.len()
                    + r.merged.len()
                    + r.conflicts.len();
                let summary = format!(
                    "sync with {}: {} sent, {} received, {} deleted on the server, {} deleted here, {} merged, {} conflicts ({} ms)",
                    self.cfg.url,
                    r.pushed.len(),
                    r.pulled.len(),
                    r.deleted_remote.len(),
                    r.deleted_local.len(),
                    r.merged.len(),
                    r.conflicts.len(),
                    took.as_millis()
                );
                if done > 0 {
                    log::info!("{summary}");
                } else {
                    log::debug!("{summary}");
                }
                for c in &r.conflict_files {
                    log::warn!(
                        "sync conflict: both sides changed, the local version is kept as {c}"
                    );
                }
                for (path, why) in &r.errors {
                    log::warn!("sync: {path} could not be synced: {why}");
                }
                for p in &r.suspect {
                    log::warn!("sync: {p} looks reformatted by another editor (lines joined)");
                }
                if before.state == "error" || before.state == "offline" {
                    log::info!("sync with {} works again", self.cfg.url);
                }
            }
            Err(e) => {
                let same = before.error_code.as_deref() == Some(api_err(e).code.as_str())
                    && before.state != "syncing";
                let line = format!("sync with {} failed: {e}", self.cfg.url);
                match (e.code(), same) {
                    (_, true) => log::debug!("{line}"),
                    ("offline", false) => log::warn!("{line}"),
                    _ => log::error!("{line}"),
                }
            }
        }
    }

    fn run_inner(&self, allow_mass_delete: bool) -> Result<notatnik_sync::Report, SyncError> {
        let password = self
            .secrets
            .get(&secret_key(&self.cfg))
            .ok_or_else(|| SyncError::Auth("the password is not known on this device".into()))?;
        let remote = WebDavRemote::new(&self.cfg.url, &self.cfg.user, &password)?;
        let mut opts = Options::new(state_file(&self.root));
        opts.trust_folder_etags = self.cfg.trust_folder_etags;
        opts.allow_mass_delete = allow_mass_delete;
        opts.bases = notatnik_sync::Bases::new(Some(bases_dir(&self.root)));
        opts.merge = self.merge.load(Ordering::Relaxed);
        notatnik_sync::sync(&self.root, &remote, &opts)
    }
}

fn worker(ctx: Arc<SyncCtx>, wake: Arc<Wake>) {
    loop {
        // wait for the timer, or for a sync that was asked for
        {
            let mut due = wake
                .due
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let default = Instant::now() + Duration::from_secs(SYNC_EVERY_SECS);
            loop {
                if wake.stop.load(Ordering::Relaxed) {
                    return;
                }
                let at = due.unwrap_or(default);
                let now = Instant::now();
                if now >= at {
                    break;
                }
                due = wake
                    .cv
                    .wait_timeout(due, at - now)
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .0;
            }
            *due = None;
        }
        if wake.stop.load(Ordering::Relaxed) {
            return;
        }
        // not while a failed login waits for a new password: asking the server again would only count as a
        // failed attempt
        if ctx.snapshot().error_code.as_deref() == Some("auth")
            && ctx.secrets.get(&secret_key(&ctx.cfg)).is_none()
        {
            continue;
        }
        ctx.run(false);
    }
}

impl AppState {
    /// Uses `secrets` for passwords instead of the process memory.
    #[must_use]
    pub fn with_secrets(mut self, secrets: Arc<dyn Secrets>) -> Self {
        self.secrets = secrets;
        self
    }

    /// Uses `client` as the calendar server instead of connecting to the real one (tests).
    #[must_use]
    pub fn with_calendar_client(
        mut self,
        client: Arc<dyn notatnik_caldav::CalDavClient + Send + Sync>,
    ) -> Self {
        self.calendar_client = Some(client);
        self
    }

    /// The sync of a graph that has a server folder: the context, and the worker where the app runs one.
    pub(super) fn start_sync(
        &self,
        graph_id: &str,
        root: &Path,
        wake: Arc<Wake>,
    ) -> Option<SyncTask> {
        let dev = notatnik_graph::DeviceSettings::load(&self.dirs).unwrap_or_default();
        let cfg = dev.remote_of(graph_id)?;
        let ctx = Arc::new(SyncCtx {
            root: root.to_path_buf(),
            status: Mutex::new(SyncStatus::idle(&cfg)),
            merge: AtomicBool::new(cfg.merge),
            cfg,
            secrets: self.secrets.clone(),
            emit: self.emit.clone(),
            gate: Mutex::new(()),
        });
        if self.watch {
            wake.kick(2);
            let (c, w) = (ctx.clone(), wake.clone());
            let _ = std::thread::Builder::new()
                .name("sync".into())
                .spawn(move || worker(c, w));
        }
        Some(SyncTask { ctx, wake })
    }

    /// The server folder of a graph, for the graph lists.
    pub(super) fn remote_of(&self, id: &str) -> Option<RemoteConfig> {
        notatnik_graph::DeviceSettings::load(&self.dirs)
            .ok()?
            .remote_of(id)
    }

    fn sync_ctx(&self) -> ApiResult<Arc<SyncCtx>> {
        self.with_open(|o| {
            o.sync
                .as_ref()
                .map(|t| t.ctx.clone())
                .ok_or_else(|| ApiError::new("invalid", "this graph is not synced with a server"))
        })
    }

    /// `sync_status`
    pub fn sync_status(&self, _a: NoArgs) -> ApiResult<SyncStatus> {
        self.with_open(|o| {
            Ok(o.sync
                .as_ref()
                .map_or_else(SyncStatus::off, |t| t.ctx.snapshot()))
        })
    }

    /// `sync_set_merge`: turns the merge of pages changed on both sides on or off for this graph.
    pub fn sync_set_merge(&self, a: SyncMergeArgs) -> ApiResult<SyncStatus> {
        let id = self.with_open(|o| Ok(o.graph.id()))?;
        let ctx = self.sync_ctx()?;
        let mut dev = notatnik_graph::DeviceSettings::load(&self.dirs).unwrap_or_default();
        if let Some(mut cfg) = dev.remote_of(&id) {
            cfg.merge = a.enabled;
            dev.set_remote(&id, Some(cfg));
            dev.save(&self.dirs)?;
        }
        ctx.merge.store(a.enabled, Ordering::Relaxed);
        Ok(ctx.set_status(|s| s.merge = a.enabled))
    }

    /// `sync_now`: syncs now and returns how it went. `allow_mass_delete` lets a sync that was stopped for wanting
    /// to delete most of the graph go ahead.
    pub fn sync_now(&self, a: SyncNowArgs) -> ApiResult<SyncStatus> {
        let ctx = self.sync_ctx()?;
        Ok(ctx.run(a.allow_mass_delete))
    }

    /// `sync_set_password`: remembers a (new) password and syncs with it.
    pub fn sync_set_password(&self, a: SyncPasswordArgs) -> ApiResult<SyncStatus> {
        let ctx = self.sync_ctx()?;
        ctx.secrets
            .set(&secret_key(&ctx.cfg), &a.password)
            .map_err(|e| {
                ApiError::new("server", format!("the password could not be stored: {e}"))
            })?;
        Ok(ctx.run(false))
    }

    /// A last sync when the app quits (SY-6): something written a moment ago should not wait for the next start.
    /// The caller bounds the wait.
    pub fn sync_on_quit(&self) {
        let ctx = {
            let g = self.lock();
            g.as_ref()
                .and_then(|o| o.sync.as_ref().map(|t| t.ctx.clone()))
        };
        if let Some(ctx) = ctx {
            if ctx.snapshot().state != "error"
                || ctx.snapshot().error_code.as_deref() == Some("files")
            {
                ctx.run(false);
            }
        }
    }

    /// `webdav_test`: looks at a server folder without changing anything.
    pub fn webdav_test(&self, a: WebdavArgs) -> ApiResult<WebdavTest> {
        use notatnik_sync::Remote;
        let url =
            notatnik_sync::folder_url(&a.server, &a.user, &a.folder).map_err(|e| api_err(&e))?;
        let remote = WebDavRemote::new(&url, &a.user, &a.password).map_err(|e| api_err(&e))?;
        let listing = remote.list("").map_err(|e| api_err(&e))?;
        let entries = listing.map(|l| l.entries);
        Ok(WebdavTest {
            url,
            exists: entries.is_some(),
            items: entries.as_ref().map_or(0, |e| e.len() as u32),
            has_graph: entries.as_ref().is_some_and(|e| looks_like_graph(e)),
        })
    }

    /// `webdav_browse`: the folders inside `folder` on the server (read-only), for the folder browser of the start
    /// screen. `folder` empty is the account's files.
    pub fn webdav_browse(&self, a: WebdavArgs) -> ApiResult<WebdavFolders> {
        use notatnik_sync::Remote;
        let url =
            notatnik_sync::folder_url(&a.server, &a.user, &a.folder).map_err(|e| api_err(&e))?;
        let remote = WebDavRemote::new(&url, &a.user, &a.password).map_err(|e| api_err(&e))?;
        let entries = remote.list("").map_err(|e| api_err(&e))?.map(|l| l.entries);
        let mut folders: Vec<String> = entries
            .iter()
            .flatten()
            .filter(|e| e.dir && !e.name.starts_with('.'))
            .map(|e| e.name.clone())
            .collect();
        folders.sort_by_key(|n| n.to_lowercase());
        let folder = a
            .folder
            .split('/')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("/");
        Ok(WebdavFolders {
            folder,
            exists: entries.is_some(),
            folders,
            items: entries.as_ref().map_or(0, |e| e.len() as u32),
            has_graph: entries.as_ref().is_some_and(|e| looks_like_graph(e)),
        })
    }

    /// `create_remote_graph`: a new graph kept in a server folder (an empty or missing folder), or an existing one
    /// attached (`attach`), downloaded into a local copy first.
    pub fn create_remote_graph(&self, a: WebdavArgs) -> ApiResult<GraphInfo> {
        use notatnik_sync::Remote;
        let url =
            notatnik_sync::folder_url(&a.server, &a.user, &a.folder).map_err(|e| api_err(&e))?;
        let remote = WebDavRemote::new(&url, &a.user, &a.password).map_err(|e| api_err(&e))?;
        let entries = remote
            .list("")
            .map_err(|e| api_err(&e))?
            .map(|l| l.entries)
            .unwrap_or_default();
        let has_graph = looks_like_graph(&entries);
        if !entries.is_empty() && !has_graph {
            return Err(ApiError::new(
                "not_a_graph",
                format!(
                    "the folder holds {} item(s) that are not a Notatnik graph; choose an empty or a new folder",
                    entries.len()
                ),
            ));
        }
        if has_graph && !a.attach {
            return Err(ApiError::new(
                "has_graph",
                "the folder already holds a Notatnik graph; attach it to download it to this device",
            ));
        }
        let probe = notatnik_sync::probe(&remote).map_err(|e| api_err(&e))?;

        // the local copy
        let name = a
            .folder
            .rsplit('/')
            .find(|s| !s.trim().is_empty())
            .unwrap_or("graph");
        let slug: String = name
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '-'
                }
            })
            .collect();
        let replicas = self.dirs.data.join("replicas");
        let mut replica = replicas.join(&slug);
        let mut n = 2;
        while replica.exists() {
            replica = replicas.join(format!("{slug}-{n}"));
            n += 1;
        }
        std::fs::create_dir_all(&replica).map_err(|e| ApiError::new("server", e.to_string()))?;
        let cleanup = |replica: &Path| {
            let _ = std::fs::remove_dir_all(replica);
            let _ = std::fs::remove_file(state_file(replica));
            let _ = std::fs::remove_dir_all(bases_dir(replica));
        };
        if has_graph {
            let mut opts = Options::new(state_file(&replica));
            opts.trust_folder_etags = probe.trusts_folder_etags;
            opts.bases = notatnik_sync::Bases::new(Some(bases_dir(&replica)));
            let report = notatnik_sync::sync(&replica, &remote, &opts).map_err(|e| {
                cleanup(&replica);
                api_err(&e)
            })?;
            if !report.errors.is_empty() {
                cleanup(&replica);
                return Err(ApiError::new(
                    "server",
                    format!("the download failed: {}", report.errors[0].1),
                ));
            }
        }
        let opened = self.open_graph(OpenGraphArgs {
            path: replica.to_string_lossy().into_owned(),
            create: !has_graph,
        });
        let info = match opened {
            Ok(i) => i,
            Err(e) => {
                cleanup(&replica);
                return Err(e);
            }
        };

        // remember the server and the password, then open again so the graph starts with its sync
        let cfg = RemoteConfig {
            url,
            user: a.user.clone(),
            trust_folder_etags: probe.trusts_folder_etags,
            merge: true,
        };
        self.secrets
            .set(&secret_key(&cfg), &a.password)
            .map_err(|e| {
                ApiError::new("server", format!("the password could not be stored: {e}"))
            })?;
        let mut dev = notatnik_graph::DeviceSettings::load(&self.dirs).unwrap_or_default();
        dev.set_remote(&info.id, Some(cfg));
        dev.save(&self.dirs)?;
        let info = self.open_graph(OpenGraphArgs {
            path: info.root,
            create: false,
        })?;
        // a new graph is uploaded now, so that the server has it when this returns
        if !has_graph {
            let _ = self.sync_now(SyncNowArgs {
                allow_mass_delete: false,
            });
        }
        Ok(info)
    }

    /// `disconnect_remote`: the graph stays on this device as a local graph; the server folder and the password
    /// are left alone (the password is forgotten).
    pub fn disconnect_remote(&self, _a: NoArgs) -> ApiResult<GraphInfo> {
        let (id, root) = self.with_open(|o| Ok((o.graph.id(), o.graph.root().to_path_buf())))?;
        let mut dev = notatnik_graph::DeviceSettings::load(&self.dirs).unwrap_or_default();
        if let Some(cfg) = dev.remote_of(&id) {
            self.secrets.delete(&secret_key(&cfg));
        }
        dev.set_remote(&id, None);
        dev.save(&self.dirs)?;
        let _ = std::fs::remove_file(state_file(&root));
        let _ = std::fs::remove_dir_all(bases_dir(&root));
        self.open_graph(OpenGraphArgs {
            path: root.to_string_lossy().into_owned(),
            create: false,
        })
    }

    /// Forgets the password of a graph that leaves the history.
    pub(super) fn forget_remote_secret(&self, id: &str) {
        if let Some(cfg) = self.remote_of(id) {
            self.secrets.delete(&secret_key(&cfg));
        }
    }
}

fn looks_like_graph(entries: &[notatnik_sync::Entry]) -> bool {
    entries
        .iter()
        .any(|e| e.dir && matches!(e.name.as_str(), ".notatnik" | "pages" | "journals"))
}
