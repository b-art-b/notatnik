//! How a file is stored on the server: as it is, encrypted as `<path>.enc` (EN-12, EN-15, EN-16), or encrypted as an
//! object with a name that reveals nothing, `.notatnik-enc/<aa>/<name>.enc`, the path being inside (EN-25).
//!
//! The engine thinks in the paths of the replica. This module knows where the server holds the copy of
//! `pages/a.md`, reads and writes the right one, and finds copies in a form or place the rules do not ask for.

use std::collections::BTreeMap;
use std::sync::Arc;

use notatnik_core::encrypted::{EncryptedPaths, hidden_name, hidden_object, remote_name};
use notatnik_crypt::Keys;

use crate::remote::{Cond, Remote};
use crate::{Error, Result};

/// What is encrypted, and the key to do it with.
pub struct Encryption {
    /// The marked paths (EN-11).
    pub paths: EncryptedPaths,
    /// The unlocked key; `None` while locked: marked files are then left alone on both sides (EN-15).
    pub keys: Option<Arc<Keys>>,
    /// Marked files are stored as objects with hidden names (EN-25), not as `<path>.enc`.
    pub hidden: bool,
}

/// The copy of a path on the server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Found {
    pub etag: String,
    pub size: u64,
    /// Stored encrypted (under `name`, which is then not the path).
    pub enc: bool,
    /// Where the copy is on the server: the path, `<path>.enc`, or a hidden object.
    pub name: String,
}

/// A copy in a form or place the rules do not ask for, next to the one they ask for (an interrupted change).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Stale {
    pub path: String,
    pub etag: String,
    pub enc: bool,
    pub name: String,
}

/// A hidden object whose path this device does not know yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Unknown {
    /// Its name on the server.
    pub object: String,
}

/// What the server lists, by the path of the replica it is the copy of: files stored as they are, files stored
/// encrypted as `<path>.enc`, and hidden objects (by their name on the server: the path is inside).
#[derive(Default)]
pub(crate) struct RemoteMaps {
    pub plain: BTreeMap<String, (String, u64)>,
    pub enc: BTreeMap<String, (String, u64)>,
    pub hidden: BTreeMap<String, (String, u64)>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Place {
    Hidden,
    Visible,
    Plain,
}

struct Copy {
    place: Place,
    etag: String,
    size: u64,
    name: String,
}

impl RemoteMaps {
    /// Adds a server file by its name there. Anything in the folder of hidden objects that is not one is not ours
    /// and is ignored.
    pub fn add(&mut self, remote_path: &str, etag: String, size: u64) {
        if hidden_name(remote_path).is_some() {
            self.hidden.insert(remote_path.to_string(), (etag, size));
        } else if remote_path.starts_with(&format!("{}/", notatnik_core::encrypted::HIDDEN_DIR)) {
            log::debug!("sync: {remote_path} is not an object of ours; ignored");
        } else {
            match notatnik_core::encrypted::plain_name(remote_path) {
                Some(p) => self.enc.insert(p.to_string(), (etag, size)),
                None => self.plain.insert(remote_path.to_string(), (etag, size)),
            };
        }
    }

    /// The hidden objects whose path `names` does not know.
    pub fn unknown(&self, names: &BTreeMap<String, String>) -> Vec<Unknown> {
        self.hidden
            .keys()
            .filter(|o| !names.contains_key(*o))
            .map(|o| Unknown { object: o.clone() })
            .collect()
    }

    /// One copy per path. `names` maps the hidden objects the device knows to their paths; the others are returned
    /// as unknown. A path with more than one copy keeps the one the rules ask for (`hidden` says whether marked
    /// files are asked to be hidden), else the most hidden one; the others are returned as stale.
    pub fn resolve(
        self,
        paths: &EncryptedPaths,
        hidden: bool,
        names: &BTreeMap<String, String>,
    ) -> (BTreeMap<String, Found>, Vec<Stale>, Vec<Unknown>) {
        let mut copies: BTreeMap<String, Vec<Copy>> = BTreeMap::new();
        let mut unknown = Vec::new();
        for (p, (etag, size)) in self.plain {
            copies.entry(p.clone()).or_default().push(Copy {
                place: Place::Plain,
                etag,
                size,
                name: p,
            });
        }
        for (p, (etag, size)) in self.enc {
            let name = remote_name(&p);
            copies.entry(p).or_default().push(Copy {
                place: Place::Visible,
                etag,
                size,
                name,
            });
        }
        for (object, (etag, size)) in self.hidden {
            match names.get(&object) {
                Some(p) => copies.entry(p.clone()).or_default().push(Copy {
                    place: Place::Hidden,
                    etag,
                    size,
                    name: object,
                }),
                None => unknown.push(Unknown { object }),
            }
        }
        let mut found = BTreeMap::new();
        let mut stale = Vec::new();
        for (p, mut list) in copies {
            let wanted = if !paths.is_encrypted(&p) {
                Place::Plain
            } else if hidden {
                Place::Hidden
            } else {
                Place::Visible
            };
            // the wanted place first, then the most hidden
            let rank = |c: &Copy| (c.place != wanted, c.place as u8);
            list.sort_by_key(rank);
            let mut it = list.into_iter();
            if let Some(keep) = it.next() {
                found.insert(
                    p.clone(),
                    Found {
                        etag: keep.etag,
                        size: keep.size,
                        enc: keep.place != Place::Plain,
                        name: keep.name,
                    },
                );
            }
            for c in it {
                stale.push(Stale {
                    path: p.clone(),
                    etag: c.etag,
                    enc: c.place != Place::Plain,
                    name: c.name,
                });
            }
        }
        (found, stale, unknown)
    }
}

/// The path inside a hidden object, if it is one of ours: opened with `keys`, and named as the path says.
pub(crate) fn open_object(keys: &Keys, object: &str, bytes: &[u8]) -> Result<(String, Vec<u8>)> {
    let (path, content) = keys
        .decrypt_named(bytes)
        .map_err(|e| Error::Crypt(format!("{object}: {e}")))?;
    if hidden_object(&keys.file_name(&path)) != object {
        return Err(Error::Crypt(format!(
            "{object} is not the object of the file it holds; nothing was used"
        )));
    }
    Ok((path, content))
}

/// Opens the hidden objects whose path is not known, to learn it. Returns the new `object -> path` pairs and the
/// objects that could not be read, with the reason. A locked key learns nothing (the objects are left alone).
pub(crate) fn learn_names(
    remote: &dyn Remote,
    keys: Option<&Keys>,
    unknown: &[Unknown],
) -> (BTreeMap<String, String>, Vec<(String, String)>) {
    let mut learned = BTreeMap::new();
    let mut failed = Vec::new();
    let Some(keys) = keys else {
        return (learned, failed);
    };
    for u in unknown {
        match remote
            .get(&u.object)
            .and_then(|(bytes, _)| open_object(keys, &u.object, &bytes))
        {
            Ok((path, _)) => {
                learned.insert(u.object.clone(), path);
            }
            Err(e) => failed.push((u.object.clone(), e.to_string())),
        }
    }
    (learned, failed)
}

/// Reads and writes files on the server in the form each one has or should have.
pub(crate) struct Io<'a> {
    pub remote: &'a dyn Remote,
    pub paths: &'a EncryptedPaths,
    pub keys: Option<&'a Keys>,
    pub found: &'a BTreeMap<String, Found>,
    /// Marked files are asked to be stored as hidden objects.
    pub hidden: bool,
}

fn crypt(e: notatnik_crypt::Error) -> Error {
    Error::Crypt(e.to_string())
}

impl Io<'_> {
    /// Whether the rules ask for `path` to be stored encrypted.
    pub fn target(&self, path: &str) -> bool {
        self.paths.is_encrypted(path)
    }

    /// Whether the copy of `path` on the server is encrypted, and for a hidden object its name (to remember).
    pub fn copy_of(&self, path: &str) -> (bool, Option<String>) {
        match self.found.get(path) {
            Some(f) => (f.enc, hidden_name(&f.name).map(|_| f.name.clone())),
            None => (false, None),
        }
    }

    fn key(&self, path: &str) -> Result<&Keys> {
        self.keys
            .ok_or_else(|| Error::Crypt(format!("{path} is encrypted and the key is locked")))
    }

    /// Where the rules ask the copy of `path` to be, and whether it is encrypted. A hidden name needs the key.
    pub fn target_name(&self, path: &str) -> Result<(String, bool)> {
        if !self.target(path) {
            return Ok((path.to_string(), false));
        }
        if self.hidden {
            let key = self.key(path)?;
            return Ok((hidden_object(&key.file_name(path)), true));
        }
        Ok((remote_name(path), true))
    }

    /// A file as the server has it now: its plain bytes and the ETag.
    pub fn get(&self, path: &str) -> Result<(Vec<u8>, String)> {
        match self.found.get(path) {
            Some(f) => self.get_at(path, &f.name, f.enc),
            None => self.remote.get(path),
        }
    }

    /// A copy at a given place.
    pub fn get_at(&self, path: &str, name: &str, enc: bool) -> Result<(Vec<u8>, String)> {
        if !enc {
            return self.remote.get(name);
        }
        let key = self.key(path)?;
        let (bytes, etag) = self.remote.get(name)?;
        if hidden_name(name).is_some() {
            let (inside, plain) = open_object(key, name, &bytes)?;
            if inside != path {
                return Err(Error::Crypt(format!("{name} holds {inside}, not {path}")));
            }
            return Ok((plain, etag));
        }
        let plain = key
            .decrypt(&bytes, path.as_bytes())
            .map_err(|e| Error::Crypt(format!("{path}: {e}")))?;
        Ok((plain, etag))
    }

    fn put_at(&self, path: &str, name: &str, data: &[u8], cond: Cond, enc: bool) -> Result<String> {
        if !enc {
            return self.remote.put(name, data, cond);
        }
        let key = self.key(path)?;
        let sealed = if hidden_name(name).is_some() {
            key.encrypt_named(path, data)
        } else {
            key.encrypt(data, path.as_bytes())
        }
        .map_err(crypt)?;
        self.remote.put(name, &sealed, cond)
    }

    /// Writes a file where the rules ask for it. `cond` is the condition for a copy that is there already; a copy
    /// elsewhere (the other form, another hidden name after the key changed) is left in place and removed
    /// afterwards (when `old_etag` is known), so a failure between the two steps leaves both, never neither. A
    /// marked file is never written as it is. Returns the new ETag, whether it is encrypted, and where it is.
    pub fn write(
        &self,
        path: &str,
        data: &[u8],
        cond: Cond,
        old_etag: Option<&str>,
    ) -> Result<(String, bool, String)> {
        let (name, enc) = self.target_name(path)?;
        match self.found.get(path) {
            Some(old) if old.name != name => {
                if let Some(dir) = name.rsplit_once('/').map(|(d, _)| d) {
                    self.remote.mkdir(dir)?;
                }
                let etag = self.put_at(path, &name, data, Cond::Create, enc)?;
                if let Err(e) = self.delete_at(&old.name, old_etag) {
                    // the next sync sees both copies and removes the one the rules do not ask for
                    log::debug!("sync: the old copy of {path} was not removed yet: {e}");
                }
                Ok((etag, enc, name))
            }
            _ => {
                if hidden_name(&name).is_some() {
                    if let Some(dir) = name.rsplit_once('/').map(|(d, _)| d) {
                        self.remote.mkdir(dir)?;
                    }
                }
                Ok((self.put_at(path, &name, data, cond, enc)?, enc, name))
            }
        }
    }

    /// Deletes the copy at `name`.
    pub fn delete_at(&self, name: &str, if_match: Option<&str>) -> Result<()> {
        self.remote.delete(name, if_match)
    }

    /// Deletes the copy of `path` the server has now.
    pub fn delete(&self, path: &str, if_match: Option<&str>) -> Result<()> {
        let name = self.found.get(path).map_or(path, |f| f.name.as_str());
        self.remote.delete(name, if_match)
    }
}
