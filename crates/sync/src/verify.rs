//! Checks the server's copy against the replica without changing anything on either side (EN-20): every file on
//! the server is downloaded, decrypted where it is stored as `<path>.enc`, and compared with the local file.

use std::collections::BTreeMap;
use std::path::Path;

use notatnik_core::encrypted::EncryptedPaths;
use notatnik_crypt::Keys;

use crate::Result;
use crate::engine::walk_remote;
use crate::forms::{Io, RemoteMaps, learn_names};
use crate::local::{self, hash};
use crate::remote::Remote;
use crate::state::SyncState;

/// What the check found. Paths are those of the replica (no `.enc`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VerifyReport {
    /// Files whose server copy has the content of the local file.
    pub same: usize,
    /// Of `same`, the files that the server holds encrypted.
    pub same_encrypted: usize,
    /// The server copy has other content than the local file (not yet synced, or damaged).
    pub differ: Vec<String>,
    /// On the server only.
    pub only_server: Vec<String>,
    /// In the replica only.
    pub only_local: Vec<String>,
    /// Could not be read or decrypted (wrong key, changed bytes, a failed download), with the reason.
    pub unreadable: Vec<(String, String)>,
    /// Stored encrypted, but not checked because no key was given.
    pub locked: Vec<String>,
    /// Marked as encrypted, yet the server holds the plain text.
    pub plain_but_marked: Vec<String>,
    /// Stored encrypted, though the rules do not ask for it any more.
    pub encrypted_but_unmarked: Vec<String>,
    /// The server holds both forms of the file.
    pub both_forms: Vec<String>,
    /// Stored encrypted, but not where the settings ask for (visible name or hidden object, EN-25, or under the
    /// name of an older key): the next sync moves it.
    pub other_place: Vec<String>,
}

impl VerifyReport {
    /// Nothing to report: every file is on both sides, with the same content, in the form the rules ask for.
    pub fn is_clean(&self) -> bool {
        self.differ.is_empty()
            && self.only_server.is_empty()
            && self.only_local.is_empty()
            && self.unreadable.is_empty()
            && self.locked.is_empty()
            && self.plain_but_marked.is_empty()
            && self.encrypted_but_unmarked.is_empty()
            && self.both_forms.is_empty()
            && self.other_place.is_empty()
    }
}

/// Compares the server folder with the replica at `root`. `keys` opens the encrypted copies; without them those
/// are listed in `locked`. Nothing is written, on either side.
pub fn verify(
    root: &Path,
    remote: &dyn Remote,
    paths: &EncryptedPaths,
    hidden: bool,
    keys: Option<&Keys>,
) -> Result<VerifyReport> {
    let state = SyncState::default();
    let mut maps = RemoteMaps::default();
    walk_remote(remote, &state, false, "", &mut maps, &mut BTreeMap::new())?;
    let mut names = BTreeMap::new();
    let (learned, failed) = learn_names(remote, keys, &maps.unknown(&names));
    names.extend(learned);
    let (found, stale, unknown) = maps.resolve(paths, hidden, &names);
    let local = local::scan(root, &state)?;
    let io = Io {
        remote,
        paths,
        keys,
        found: &found,
        hidden,
    };
    let mut r = VerifyReport {
        both_forms: stale.into_iter().map(|s| s.path).collect(),
        unreadable: failed,
        ..VerifyReport::default()
    };
    // objects with a hidden name whose path cannot be read without the key
    if keys.is_none() {
        r.locked.extend(unknown.into_iter().map(|u| u.object));
    }
    for (path, f) in &found {
        let Some(mine) = local.get(path) else {
            r.only_server.push(path.clone());
            continue;
        };
        if f.enc && keys.is_none() {
            r.locked.push(path.clone());
            continue;
        }
        match (f.enc, io.target(path)) {
            (false, true) => r.plain_but_marked.push(path.clone()),
            (true, false) => r.encrypted_but_unmarked.push(path.clone()),
            _ => {}
        }
        if f.enc && io.target(path) && io.target_name(path).is_ok_and(|(n, _)| n != f.name) {
            r.other_place.push(path.clone());
        }
        match io.get(path) {
            Ok((bytes, _)) if hash(&bytes) == mine.hash => {
                r.same += 1;
                r.same_encrypted += usize::from(f.enc);
            }
            Ok(_) => r.differ.push(path.clone()),
            Err(e) => r.unreadable.push((path.clone(), e.to_string())),
        }
    }
    r.only_local = local
        .keys()
        .filter(|p| !found.contains_key(*p) && !p.ends_with(".enc"))
        .cloned()
        .collect();
    r.both_forms.sort();
    Ok(r)
}
