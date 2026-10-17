//! The sync: scan both sides, plan, do it, repeat until nothing is left to do.

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

use chrono::NaiveDateTime;

use crate::local::{self, hash, ignored, mtime_ms, to_trash, write_atomic};
use crate::plan::{Action, plan};
use crate::remote::{Cond, Remote};
use crate::state::{FileState, SyncState};
use crate::{Error, Result};

/// How to sync.
pub struct Options {
    /// Where the sync state is kept (outside the replica).
    pub state_path: PathBuf,
    /// The server's folder ETags change whenever something below changes (see [`crate::Probe`]): folders whose ETag
    /// is as at the last sync are not listed again.
    pub trust_folder_etags: bool,
    /// Let one sync delete many files at once. Off, a sync that would delete more than ten files and more than
    /// half of the known ones stops with [`Error::MassDelete`]: an emptied folder or a wiped server must not
    /// spread.
    pub allow_mass_delete: bool,
    /// The clock, for the names of conflict copies.
    pub now: Box<dyn Fn() -> NaiveDateTime + Send + Sync>,
}

impl Options {
    /// Default options for a state file.
    pub fn new(state_path: PathBuf) -> Options {
        Options {
            state_path,
            trust_folder_etags: false,
            allow_mass_delete: false,
            now: Box::new(|| chrono::Local::now().naive_local()),
        }
    }
}

/// What a sync did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Report {
    /// Uploaded.
    pub pushed: Vec<String>,
    /// Downloaded.
    pub pulled: Vec<String>,
    /// Deleted on the server.
    pub deleted_remote: Vec<String>,
    /// Moved to the local trash.
    pub deleted_local: Vec<String>,
    /// Files changed on both sides: the server's version is at the path, the local one next to it as a
    /// "conflicted copy".
    pub conflicts: Vec<String>,
    /// The conflict copies that were written.
    pub conflict_files: Vec<String>,
    /// Files that failed, with the reason; the next sync tries them again.
    pub errors: Vec<(String, String)>,
    /// Everything is in step: nothing was left to do at the end.
    pub complete: bool,
}

impl Report {
    /// Anything moved.
    pub fn changed(&self) -> bool {
        !(self.pushed.is_empty()
            && self.pulled.is_empty()
            && self.deleted_remote.is_empty()
            && self.deleted_local.is_empty()
            && self.conflicts.is_empty())
    }
}

/// `name (conflicted copy 2026-10-17 123000).ext` for `dir/name.ext`.
fn conflict_name(path: &str, now: NaiveDateTime, n: usize) -> String {
    let (dir, file) = path.rsplit_once('/').map_or(("", path), |(d, f)| (d, f));
    let (stem, ext) = file.rsplit_once('.').map_or((file, ""), |(s, e)| (s, e));
    let stamp = now.format("%Y-%m-%d %H%M%S");
    let more = if n == 0 {
        String::new()
    } else {
        format!(" {n}")
    };
    let mut out = String::new();
    if !dir.is_empty() {
        out.push_str(dir);
        out.push('/');
    }
    out.push_str(&format!("{stem} (conflicted copy {stamp}{more})"));
    if !ext.is_empty() {
        out.push('.');
        out.push_str(ext);
    }
    out
}

type RemoteFiles = BTreeMap<String, (String, u64)>;

fn scan_remote(
    remote: &dyn Remote,
    state: &SyncState,
    trust: bool,
) -> Result<(RemoteFiles, BTreeMap<String, String>)> {
    let mut files = BTreeMap::new();
    let mut dirs = BTreeMap::new();
    walk_remote(remote, state, trust, "", &mut files, &mut dirs)?;
    Ok((files, dirs))
}

/// Copies what the state knows about the folder `dir` and everything below it (it has not changed since).
fn reuse(
    state: &SyncState,
    dir: &str,
    files: &mut RemoteFiles,
    dirs: &mut BTreeMap<String, String>,
) {
    let prefix = if dir.is_empty() {
        String::new()
    } else {
        format!("{dir}/")
    };
    for (p, f) in state.files.range(prefix.clone()..) {
        if !p.starts_with(&prefix) {
            break;
        }
        files.insert(p.clone(), (f.etag.clone(), f.size));
    }
    for (d, e) in state.dirs.range(prefix.clone()..) {
        if !d.starts_with(&prefix) {
            break;
        }
        dirs.insert(d.clone(), e.clone());
    }
}

fn walk_remote(
    remote: &dyn Remote,
    state: &SyncState,
    trust: bool,
    dir: &str,
    files: &mut RemoteFiles,
    dirs: &mut BTreeMap<String, String>,
) -> Result<()> {
    let Some(l) = remote.list(dir)? else {
        return Ok(());
    };
    let same = |path: &str, etag: &str| {
        trust && !etag.is_empty() && state.dirs.get(path).map(String::as_str) == Some(etag)
    };
    dirs.insert(dir.to_string(), l.etag.clone());
    if dir.is_empty() && same("", &l.etag) {
        // nothing below changed since the last complete sync: what it held then is what it holds now
        reuse(state, "", files, dirs);
        return Ok(());
    }
    let prefix = if dir.is_empty() {
        String::new()
    } else {
        format!("{dir}/")
    };
    for e in l.entries {
        let path = format!("{prefix}{}", e.name);
        if e.dir {
            if ignored(&format!("{path}/")) {
                continue;
            }
            if same(&path, &e.etag) {
                // a folder's ETag is in the listing of its parent: no need to ask the folder itself
                dirs.insert(path.clone(), e.etag.clone());
                reuse(state, &path, files, dirs);
            } else {
                walk_remote(remote, state, trust, &path, files, dirs)?;
            }
        } else if !ignored(&path) {
            files.insert(path, (e.etag, e.size));
        }
    }
    Ok(())
}

/// Brings the replica at `root` and the remote folder in step, as far as possible. Conflicts do not stop it: the
/// server's version is kept at the path and the local one is saved beside it (SY-4). Errors that concern one file
/// are collected in the report; the login failing or the server being unreachable stops the sync.
pub fn sync(root: &Path, remote: &dyn Remote, opts: &Options) -> Result<Report> {
    let mut state = SyncState::load(&opts.state_path);
    let mut report = Report::default();
    let mut made_dirs: HashSet<String> = HashSet::new();
    for _pass in 0..5 {
        let local = local::scan(root, &state)?;
        let (rfiles, rdirs) = scan_remote(remote, &state, opts.trust_folder_etags)?;
        let actions = plan(&local, &rfiles, &state.files);
        if actions.is_empty() {
            if opts.trust_folder_etags {
                state.dirs = rdirs;
            }
            if report.errors.is_empty() {
                state.last_sync = Some(chrono::Local::now().to_rfc3339());
                report.complete = true;
            }
            state.save(&opts.state_path)?;
            break;
        }
        let deletes = actions
            .iter()
            .filter(|a| matches!(a, Action::DeleteLocal { .. } | Action::DeleteRemote { .. }))
            .count();
        if !opts.allow_mass_delete && deletes > 10 && deletes * 2 > state.files.len() {
            return Err(Error::MassDelete(deletes));
        }
        let mut progress = false;
        for a in &actions {
            match run(
                root,
                remote,
                opts,
                &mut state,
                &local,
                &mut made_dirs,
                a,
                &mut report,
            ) {
                Ok(done) => progress |= done,
                Err(e @ (Error::Auth(_) | Error::Offline(_) | Error::Url(_))) => {
                    state.save(&opts.state_path)?;
                    return Err(e);
                }
                Err(e) => report.errors.push((a.path().to_string(), e.to_string())),
            }
        }
        state.save(&opts.state_path)?;
        if !progress {
            break;
        }
    }
    Ok(report)
}

#[allow(clippy::too_many_arguments)]
fn run(
    root: &Path,
    remote: &dyn Remote,
    opts: &Options,
    state: &mut SyncState,
    local: &BTreeMap<String, local::LocalFile>,
    made_dirs: &mut HashSet<String>,
    action: &Action,
    report: &mut Report,
) -> Result<bool> {
    match action {
        Action::Forget { path } => {
            state.files.remove(path);
            Ok(true)
        }
        Action::Push { path, etag } => {
            let abs = root.join(path);
            let Ok(md) = std::fs::metadata(&abs) else {
                return Ok(false);
            };
            let Ok(bytes) = std::fs::read(&abs) else {
                return Ok(false);
            };
            let parent = path.rsplit_once('/').map_or("", |(p, _)| p);
            if made_dirs.insert(parent.to_string()) {
                remote.mkdir(parent)?;
            }
            let cond = match etag {
                Some(e) => Cond::Match(e.clone()),
                None => Cond::Create,
            };
            match remote.put(path, &bytes, cond) {
                Ok(new_etag) => {
                    state.files.insert(
                        path.clone(),
                        FileState {
                            hash: hash(&bytes),
                            etag: new_etag,
                            size: bytes.len() as u64,
                            mtime_ms: mtime_ms(&md),
                        },
                    );
                    report.pushed.push(path.clone());
                    Ok(true)
                }
                // the server changed meanwhile: the next pass sees both sides changed
                Err(Error::Changed(_)) => Ok(false),
                Err(e) => Err(e),
            }
        }
        Action::Pull { path } => {
            if let Some(l) = local.get(path) {
                if !unchanged(root, path, &l.hash) {
                    return Err(Error::Changed(format!("{path} was edited while syncing")));
                }
            }
            let (bytes, etag) = remote.get(path)?;
            // the server gave a file a new ETag without changing it (Nextcloud sometimes does after an upload):
            // remember the new ETag, leave the file alone
            if let Some(l) = local.get(path).filter(|l| l.hash == hash(&bytes)) {
                state.files.insert(
                    path.clone(),
                    FileState {
                        hash: l.hash.clone(),
                        etag,
                        size: l.size,
                        mtime_ms: l.mtime_ms,
                    },
                );
                return Ok(true);
            }
            let mtime = write_atomic(root, path, &bytes)?;
            state.files.insert(
                path.clone(),
                FileState {
                    hash: hash(&bytes),
                    etag,
                    size: bytes.len() as u64,
                    mtime_ms: mtime,
                },
            );
            report.pulled.push(path.clone());
            Ok(true)
        }
        Action::DeleteRemote { path, etag } => {
            match remote.delete(path, Some(etag)) {
                Ok(()) => {}
                // it changed there meanwhile: the next pass pulls it
                Err(Error::Changed(_)) => return Ok(false),
                Err(e) => return Err(e),
            }
            state.files.remove(path);
            report.deleted_remote.push(path.clone());
            Ok(true)
        }
        Action::DeleteLocal { path } => {
            if let Some(l) = local.get(path) {
                if !unchanged(root, path, &l.hash) {
                    return Err(Error::Changed(format!("{path} was edited while syncing")));
                }
            }
            if root.join(path).exists() {
                to_trash(root, path)?;
            }
            state.files.remove(path);
            report.deleted_local.push(path.clone());
            Ok(true)
        }
        Action::Reconcile { path } => {
            let (bytes, etag) = remote.get(path)?;
            let remote_hash = hash(&bytes);
            let abs = root.join(path);
            let mine = std::fs::read(&abs).ok();
            let mut copy = None;
            if let Some(mine) = &mine {
                if hash(mine) != remote_hash {
                    // both changed: the server's version stays at the path, mine is kept beside it
                    let now = (opts.now)();
                    let mut n = 0;
                    let name = loop {
                        let c = conflict_name(path, now, n);
                        if !root.join(&c).exists() {
                            break c;
                        }
                        n += 1;
                    };
                    write_atomic(root, &name, mine)?;
                    copy = Some(name);
                }
            }
            let mtime = if mine.as_ref().is_some_and(|m| hash(m) == remote_hash) {
                std::fs::metadata(&abs).map_or(0, |m| mtime_ms(&m))
            } else {
                write_atomic(root, path, &bytes)?
            };
            state.files.insert(
                path.clone(),
                FileState {
                    hash: remote_hash,
                    etag,
                    size: bytes.len() as u64,
                    mtime_ms: mtime,
                },
            );
            if let Some(c) = copy {
                report.conflicts.push(path.clone());
                report.conflict_files.push(c);
            }
            Ok(true)
        }
    }
}

/// The local file still has the content that was scanned.
fn unchanged(root: &Path, path: &str, scanned: &str) -> bool {
    std::fs::read(root.join(path)).is_ok_and(|b| hash(&b) == scanned)
}
