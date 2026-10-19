//! The sync: scan both sides, plan, do it, repeat until nothing is left to do.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::{Path, PathBuf};

use chrono::NaiveDateTime;

use notatnik_core::encrypted::{EncryptedPaths, plain_name};

use crate::Bases;
use crate::forms::{Encryption, Io, RemoteMaps};
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
    /// Where the copies of the synced pages are kept (see [`Bases`]); `None`: no merging.
    pub bases: Bases,
    /// Merge a page that changed on both sides when the merge is clean (needs `bases`); otherwise, and always when
    /// the merge has conflicts, the local version is kept as a "conflicted copy".
    pub merge: bool,
    /// The clock, for the names of conflict copies.
    pub now: Box<dyn Fn() -> NaiveDateTime + Send + Sync>,
    /// Which files are stored encrypted on the server and the key for them (EN-11 to EN-16); `None`: none are, and
    /// encrypted copies that the server holds are left alone.
    pub encryption: Option<Encryption>,
}

impl Options {
    /// Default options for a state file.
    pub fn new(state_path: PathBuf) -> Options {
        Options {
            state_path,
            trust_folder_etags: false,
            allow_mass_delete: false,
            bases: Bases::default(),
            merge: true,
            now: Box::new(|| chrono::Local::now().naive_local()),
            encryption: None,
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
    /// Pages changed on both sides that were merged without a conflict (the result is on both sides).
    pub merged: Vec<String>,
    /// Files changed on both sides: the server's version is at the path, the local one next to it as a
    /// "conflicted copy".
    pub conflicts: Vec<String>,
    /// The conflict copies that were written.
    pub conflict_files: Vec<String>,
    /// Files that failed, with the reason; the next sync tries them again.
    pub errors: Vec<(String, String)>,
    /// Downloaded pages whose lines look joined together by some other editor (a block ID followed by properties on
    /// the same line, which this app never writes).
    pub suspect: Vec<String>,
    /// Files stored in another form than the rules ask for (as it is or encrypted) that were stored again (EN-16).
    pub recoded: Vec<String>,
    /// Encrypted files left alone because the key is locked (EN-15): neither sent, received nor deleted.
    pub left_alone: usize,
    /// Local files whose name ends in `.enc`: the ending is reserved for encrypted copies, so they are not synced.
    pub reserved: Vec<String>,
    /// Everything is in step: nothing was left to do at the end.
    pub complete: bool,
}

/// True for the text of a page in which a block ID is followed by a `key:: value` property on the same line
/// (`- TODO call ^abc123 deadline:: 2026-10-21`). The app puts properties on lines of their own; a file like this was
/// rewritten by an editor that joins the lines of a list item (Nextcloud's Text editor does).
pub fn looks_reformatted(text: &str) -> bool {
    text.lines().any(|line| {
        let mut from = 0;
        while let Some(at) = line[from..].find(" ^") {
            let rest = &line[from + at + 2..];
            from += at + 2;
            let id: String = rest.chars().take(6).collect();
            let after = &rest[id.len()..];
            if id.len() == 6
                && id
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
                && after.starts_with(' ')
            {
                let word: String = after
                    .trim_start()
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
                    .collect();
                if !word.is_empty() && after.trim_start()[word.len()..].starts_with("::") {
                    return true;
                }
            }
        }
        false
    })
}

impl Report {
    /// Records a file that failed. A file that fails again in the next pass of the same sync is listed once.
    fn add_error(&mut self, path: &str, message: String) {
        let entry = (path.to_string(), message);
        if !self.errors.contains(&entry) {
            self.errors.push(entry);
        }
    }

    /// Anything moved.
    pub fn changed(&self) -> bool {
        !(self.pushed.is_empty()
            && self.pulled.is_empty()
            && self.deleted_remote.is_empty()
            && self.deleted_local.is_empty()
            && self.merged.is_empty()
            && self.recoded.is_empty()
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

/// A copy of a file in the form the rules do not ask for goes when it is the same as the one they ask for. When the
/// two differ nothing is deleted: the person decides which to keep.
fn remove_stale(io: &Io, stale: &[crate::forms::Stale], report: &mut Report) {
    for s in stale {
        let same = (|| -> Result<bool> {
            let kept = io.get(&s.path)?.0;
            let other = io.get_form(&s.path, s.enc)?.0;
            Ok(kept == other)
        })();
        match same {
            Ok(true) => match io.delete(&s.path, s.enc, Some(&s.etag)) {
                Ok(()) => log::debug!("sync: removed the second copy of {}", s.path),
                Err(Error::Changed(_)) => {}
                Err(e) => report.add_error(&s.path, e.to_string()),
            },
            Ok(false) => report.add_error(
                &s.path,
                "the server holds it both as it is and encrypted, with different contents; nothing was deleted, keep one by hand".into(),
            ),
            // locked, or not readable: try again when it is
            Err(_) => {}
        }
    }
}

fn scan_remote(
    remote: &dyn Remote,
    state: &SyncState,
    trust: bool,
) -> Result<(RemoteMaps, BTreeMap<String, String>)> {
    let mut files = RemoteMaps::default();
    let mut dirs = BTreeMap::new();
    walk_remote(remote, state, trust, "", &mut files, &mut dirs)?;
    Ok((files, dirs))
}

/// Copies what the state knows about the folder `dir` and everything below it (it has not changed since).
fn reuse(
    state: &SyncState,
    dir: &str,
    files: &mut RemoteMaps,
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
        let map = if f.enc {
            &mut files.enc
        } else {
            &mut files.plain
        };
        map.insert(p.clone(), (f.etag.clone(), f.size));
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
    files: &mut RemoteMaps,
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
            files.add(&path, e.etag, e.size);
        }
    }
    Ok(())
}

/// Brings the replica at `root` and the remote folder in step, as far as possible. Conflicts do not stop it: the
/// server's version is kept at the path and the local one is saved beside it (SY-4). Errors that concern one file
/// are collected in the report; the login failing or the server being unreachable stops the sync.
pub fn sync(root: &Path, remote: &dyn Remote, opts: &Options) -> Result<Report> {
    let none = EncryptedPaths::default();
    let paths = opts.encryption.as_ref().map_or(&none, |e| &e.paths);
    let keys = opts.encryption.as_ref().and_then(|e| e.keys.as_deref());
    let mut state = SyncState::load(&opts.state_path);
    let mut report = Report::default();
    let mut made_dirs: HashSet<String> = HashSet::new();
    for _pass in 0..5 {
        let mut local = local::scan(root, &state)?;
        // `.enc` is the ending of encrypted copies on the server, so a local file with it is not synced
        for p in local
            .keys()
            .filter(|p| plain_name(p).is_some())
            .cloned()
            .collect::<Vec<_>>()
        {
            local.remove(&p);
            if !report.reserved.contains(&p) {
                report.reserved.push(p);
            }
        }
        // pages that are as they were at the last sync are the base of a later merge (also for files that were
        // synced before bases were kept)
        if opts.merge {
            for (path, l) in &local {
                if path.ends_with(".md")
                    && state.files.get(path).is_some_and(|f| f.hash == l.hash)
                    && !opts.bases.has_synced(path)
                    && let Ok(bytes) = std::fs::read(root.join(path))
                    && hash(&bytes) == l.hash
                {
                    opts.bases.save_synced(path, &bytes);
                }
            }
        }
        let (maps, rdirs) = scan_remote(remote, &state, opts.trust_folder_etags)?;
        let (found, stale) = maps.resolve(paths);
        let io = Io {
            remote,
            paths,
            keys,
            found: &found,
        };
        remove_stale(&io, &stale, &mut report);
        // while the key is locked, every file that is or should be encrypted is left alone on both sides: no
        // action, nothing counted for the guard below (EN-15)
        let left: BTreeSet<String> = if keys.is_some() {
            BTreeSet::new()
        } else {
            local
                .keys()
                .chain(found.keys())
                .chain(state.files.keys())
                .filter(|p| {
                    paths.is_encrypted(p)
                        || found.get(*p).is_some_and(|f| f.enc)
                        || state.files.get(*p).is_some_and(|f| f.enc)
                })
                .cloned()
                .collect()
        };
        report.left_alone = left.len();
        let local_view: BTreeMap<String, local::LocalFile> = local
            .iter()
            .filter(|(p, _)| !left.contains(*p))
            .map(|(p, l)| (p.clone(), l.clone()))
            .collect();
        let rfiles: RemoteFiles = found
            .iter()
            .filter(|(p, _)| !left.contains(*p))
            .map(|(p, f)| (p.clone(), (f.etag.clone(), f.size)))
            .collect();
        let files_view: BTreeMap<String, FileState> = state
            .files
            .iter()
            .filter(|(p, _)| !left.contains(*p))
            .map(|(p, f)| (p.clone(), f.clone()))
            .collect();
        let mut actions = plan(&local_view, &rfiles, &files_view);
        // the same on both sides, but the server holds it in the other form than the rules ask for
        let planned: BTreeSet<String> = actions.iter().map(|a| a.path().to_string()).collect();
        for (p, f) in &found {
            if !left.contains(p)
                && !planned.contains(p)
                && f.enc != paths.is_encrypted(p)
                && local_view.contains_key(p)
                && files_view.contains_key(p)
            {
                actions.push(Action::Recode {
                    path: p.clone(),
                    etag: f.etag.clone(),
                });
            }
        }
        if actions.is_empty() {
            if opts.trust_folder_etags {
                // folders are skipped at the next sync only when everything in them was dealt with: a file that
                // was left alone (locked key) or could not be dealt with must be seen again, not hidden by the ETag
                if left.is_empty() && report.errors.is_empty() {
                    state.dirs = rdirs;
                } else {
                    state.dirs.clear();
                }
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
        if !opts.allow_mass_delete && deletes > 10 && deletes * 2 > files_view.len() {
            log::warn!(
                "sync stopped: it would delete {deletes} of {} files; confirm in the app to go on",
                files_view.len()
            );
            return Err(Error::MassDelete(deletes));
        }
        let mut progress = false;
        for a in &actions {
            match run(
                root,
                &io,
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
                Err(e) => report.add_error(a.path(), e.to_string()),
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
    io: &Io,
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
                io.remote.mkdir(parent)?;
            }
            let cond = match etag {
                Some(e) => Cond::Match(e.clone()),
                None => Cond::Create,
            };
            match io.write(path, &bytes, cond, etag.as_deref()) {
                Ok((new_etag, enc)) => {
                    opts.bases.save_synced(path, &bytes);
                    state.files.insert(
                        path.clone(),
                        FileState {
                            hash: hash(&bytes),
                            etag: new_etag,
                            size: bytes.len() as u64,
                            mtime_ms: mtime_ms(&md),
                            enc,
                        },
                    );
                    log::debug!("sync: sent {path}");
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
            let enc = io.form(path).unwrap_or(false);
            let (bytes, etag) = io.get(path)?;
            // the server gave a file a new ETag without changing it (Nextcloud sometimes does after an upload):
            // remember the new ETag, leave the file alone
            if let Some(l) = local.get(path).filter(|l| l.hash == hash(&bytes)) {
                opts.bases.save_synced(path, &bytes);
                state.files.insert(
                    path.clone(),
                    FileState {
                        hash: l.hash.clone(),
                        etag,
                        size: l.size,
                        mtime_ms: l.mtime_ms,
                        enc,
                    },
                );
                return Ok(true);
            }
            let mtime = write_atomic(root, path, &bytes)?;
            opts.bases.save_synced(path, &bytes);
            state.files.insert(
                path.clone(),
                FileState {
                    hash: hash(&bytes),
                    etag,
                    size: bytes.len() as u64,
                    mtime_ms: mtime,
                    enc,
                },
            );
            log::debug!("sync: received {path}");
            report.pulled.push(path.clone());
            if path.ends_with(".md") && std::str::from_utf8(&bytes).is_ok_and(looks_reformatted) {
                report.suspect.push(path.clone());
            }
            Ok(true)
        }
        Action::DeleteRemote { path, etag } => {
            match io.delete(path, io.form(path).unwrap_or(false), Some(etag)) {
                Ok(()) => {}
                // it changed there meanwhile: the next pass pulls it
                Err(Error::Changed(_)) => return Ok(false),
                Err(e) => return Err(e),
            }
            state.files.remove(path);
            opts.bases.remove_synced(path);
            log::debug!("sync: deleted {path} on the server");
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
            opts.bases.remove_synced(path);
            log::info!("sync: {path} was deleted on the server; moved to the local trash");
            report.deleted_local.push(path.clone());
            Ok(true)
        }
        Action::Recode { path, etag } => {
            let abs = root.join(path);
            let (Ok(md), Ok(bytes)) = (std::fs::metadata(&abs), std::fs::read(&abs)) else {
                return Ok(false);
            };
            if state.files.get(path).is_none_or(|f| f.hash != hash(&bytes)) {
                return Err(Error::Changed(format!("{path} was edited while syncing")));
            }
            // the server copy is read back first: only what is on the server now may be replaced
            let (theirs, _) = io.get(path)?;
            if theirs != bytes {
                return Ok(false);
            }
            let (new_etag, enc) = io.write(path, &bytes, Cond::Match(etag.clone()), Some(etag))?;
            state.files.insert(
                path.clone(),
                FileState {
                    hash: hash(&bytes),
                    etag: new_etag,
                    size: bytes.len() as u64,
                    mtime_ms: mtime_ms(&md),
                    enc,
                },
            );
            log::info!(
                "sync: {path} is stored {} on the server now",
                if enc { "encrypted" } else { "as it is" }
            );
            report.recoded.push(path.clone());
            Ok(true)
        }
        Action::Reconcile { path } => {
            let enc = io.form(path).unwrap_or(false);
            let (bytes, etag) = io.get(path)?;
            let remote_hash = hash(&bytes);
            let abs = root.join(path);
            let mine = std::fs::read(&abs).ok();
            let mut copy = None;
            if let Some(mine) = &mine {
                if hash(mine) != remote_hash {
                    // the page as both sides last had it, if it is known and is what the state says
                    let base = opts
                        .bases
                        .synced(path)
                        .filter(|b| state.files.get(path).is_some_and(|f| f.hash == hash(b)));
                    if opts.merge
                        && let Some(base) = &base
                        && let Some(merged) = merged_page(base, mine, &bytes)
                    {
                        let (md, enc) =
                            match io.write(path, &merged, Cond::Match(etag.clone()), Some(&etag)) {
                                Ok(done) => done,
                                // the server changed again: the next pass starts over from the newer version
                                Err(Error::Changed(_)) => return Ok(false),
                                Err(e) => return Err(e),
                            };
                        let mtime = write_atomic(root, path, &merged)?;
                        opts.bases.save_synced(path, &merged);
                        state.files.insert(
                            path.clone(),
                            FileState {
                                hash: hash(&merged),
                                etag: md,
                                size: merged.len() as u64,
                                mtime_ms: mtime,
                                enc,
                            },
                        );
                        log::info!("sync: both sides changed {path}; merged without a conflict");
                        report.merged.push(path.clone());
                        return Ok(true);
                    }
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
                    if let Some(base) = &base {
                        opts.bases.save_ancestor(&name, base);
                    }
                    copy = Some(name);
                }
            }
            let mtime = if mine.as_ref().is_some_and(|m| hash(m) == remote_hash) {
                std::fs::metadata(&abs).map_or(0, |m| mtime_ms(&m))
            } else {
                write_atomic(root, path, &bytes)?
            };
            opts.bases.save_synced(path, &bytes);
            state.files.insert(
                path.clone(),
                FileState {
                    hash: remote_hash,
                    etag,
                    size: bytes.len() as u64,
                    mtime_ms: mtime,
                    enc,
                },
            );
            if let Some(c) = copy {
                log::warn!(
                    "sync: both sides changed {path}; the server version is kept, yours is {c}"
                );
                report.conflicts.push(path.clone());
                report.conflict_files.push(c);
            }
            Ok(true)
        }
    }
}

/// The merge of two versions of a page, when it is clean and parses as well as the inputs did.
fn merged_page(base: &[u8], ours: &[u8], theirs: &[u8]) -> Option<Vec<u8>> {
    let (b, o, t) = (
        std::str::from_utf8(base).ok()?,
        std::str::from_utf8(ours).ok()?,
        std::str::from_utf8(theirs).ok()?,
    );
    let r = notatnik_core::merge(b, o, t).ok()?;
    let diags = |s: &str| notatnik_core::parse_page(s).diagnostics.len();
    (r.is_clean() && diags(&r.merged_text) <= diags(o).max(diags(t)).max(diags(b)))
        .then(|| r.merged_text.into_bytes())
}

/// The local file still has the content that was scanned.
fn unchanged(root: &Path, path: &str, scanned: &str) -> bool {
    std::fs::read(root.join(path)).is_ok_and(|b| hash(&b) == scanned)
}
