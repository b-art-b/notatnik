//! How a file is stored on the server: as it is, or encrypted as `<path>.enc` (EN-12, EN-15, EN-16).
//!
//! The engine thinks in the paths of the replica. This module knows that the server holds the copy of `pages/a.md`
//! under `pages/a.md` or under `pages/a.md.enc`, reads and writes the right one, and finds copies in the form the
//! rules do not ask for.

use std::collections::BTreeMap;
use std::sync::Arc;

use notatnik_core::encrypted::{EncryptedPaths, remote_name};
use notatnik_crypt::Keys;

use crate::remote::{Cond, Remote};
use crate::{Error, Result};

/// What is encrypted, and the key to do it with.
pub struct Encryption {
    /// The marked paths (EN-11).
    pub paths: EncryptedPaths,
    /// The unlocked key; `None` while locked: marked files are then left alone on both sides (EN-15).
    pub keys: Option<Arc<Keys>>,
}

/// The copy of a path on the server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Found {
    pub etag: String,
    pub size: u64,
    /// Stored as `<path>.enc`.
    pub enc: bool,
}

/// A copy in the form the rules do not ask for, next to the one they ask for (an interrupted change of form).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Stale {
    pub path: String,
    pub etag: String,
    pub enc: bool,
}

/// What the server lists, by the path of the replica it is the copy of: files stored as they are, and files stored
/// encrypted.
#[derive(Default)]
pub(crate) struct RemoteMaps {
    pub plain: BTreeMap<String, (String, u64)>,
    pub enc: BTreeMap<String, (String, u64)>,
}

impl RemoteMaps {
    /// Adds a server file by its name there.
    pub fn add(&mut self, remote_path: &str, etag: String, size: u64) {
        match notatnik_core::encrypted::plain_name(remote_path) {
            Some(p) => self.enc.insert(p.to_string(), (etag, size)),
            None => self.plain.insert(remote_path.to_string(), (etag, size)),
        };
    }

    /// One copy per path. A path stored in both forms keeps the form the rules ask for; the other is returned as
    /// stale.
    pub fn resolve(self, paths: &EncryptedPaths) -> (BTreeMap<String, Found>, Vec<Stale>) {
        let mut found = BTreeMap::new();
        let mut stale = Vec::new();
        let RemoteMaps { mut plain, mut enc } = self;
        for (p, (etag, size)) in std::mem::take(&mut plain) {
            if let Some((e2, s2)) = enc.remove(&p) {
                let want_enc = paths.is_encrypted(&p);
                let (keep, drop) = if want_enc {
                    ((e2, s2, true), (etag, false))
                } else {
                    ((etag, size, false), (e2, true))
                };
                found.insert(
                    p.clone(),
                    Found {
                        etag: keep.0,
                        size: keep.1,
                        enc: keep.2,
                    },
                );
                stale.push(Stale {
                    path: p,
                    etag: drop.0,
                    enc: drop.1,
                });
            } else {
                found.insert(
                    p,
                    Found {
                        etag,
                        size,
                        enc: false,
                    },
                );
            }
        }
        for (p, (etag, size)) in enc {
            found.insert(
                p,
                Found {
                    etag,
                    size,
                    enc: true,
                },
            );
        }
        (found, stale)
    }
}

/// Reads and writes files on the server in the form each one has or should have.
pub(crate) struct Io<'a> {
    pub remote: &'a dyn Remote,
    pub paths: &'a EncryptedPaths,
    pub keys: Option<&'a Keys>,
    pub found: &'a BTreeMap<String, Found>,
}

fn crypt(e: notatnik_crypt::Error) -> Error {
    Error::Crypt(e.to_string())
}

impl Io<'_> {
    /// Whether the rules ask for `path` to be stored encrypted.
    pub fn target(&self, path: &str) -> bool {
        self.paths.is_encrypted(path)
    }

    /// The form of the copy the server has now.
    pub fn form(&self, path: &str) -> Option<bool> {
        self.found.get(path).map(|f| f.enc)
    }

    fn key(&self, path: &str) -> Result<&Keys> {
        self.keys
            .ok_or_else(|| Error::Crypt(format!("{path} is encrypted and the key is locked")))
    }

    /// A file as the server has it now: its plain bytes and the ETag.
    pub fn get(&self, path: &str) -> Result<(Vec<u8>, String)> {
        self.get_form(path, self.form(path).unwrap_or(false))
    }

    /// A copy in a given form.
    pub fn get_form(&self, path: &str, enc: bool) -> Result<(Vec<u8>, String)> {
        if !enc {
            return self.remote.get(path);
        }
        let key = self.key(path)?;
        let (bytes, etag) = self.remote.get(&remote_name(path))?;
        let plain = key
            .decrypt(&bytes, path.as_bytes())
            .map_err(|e| Error::Crypt(format!("{path}: {e}")))?;
        Ok((plain, etag))
    }

    fn put_form(&self, path: &str, data: &[u8], cond: Cond, enc: bool) -> Result<String> {
        if !enc {
            return self.remote.put(path, data, cond);
        }
        let sealed = self
            .key(path)?
            .encrypt(data, path.as_bytes())
            .map_err(crypt)?;
        self.remote.put(&remote_name(path), &sealed, cond)
    }

    /// Writes a file in the form the rules ask for. `cond` is the condition for a copy that has that form already;
    /// a copy in the other form is left in place and removed afterwards (when `old_etag` is known), so a failure
    /// between the two steps leaves both forms, never neither. A marked file is never written as it is.
    pub fn write(
        &self,
        path: &str,
        data: &[u8],
        cond: Cond,
        old_etag: Option<&str>,
    ) -> Result<(String, bool)> {
        let target = self.target(path);
        match self.form(path) {
            Some(old) if old != target => {
                let etag = self.put_form(path, data, Cond::Create, target)?;
                if let Err(e) = self.delete(path, old, old_etag) {
                    // the next sync sees both forms and removes the one the rules do not ask for
                    log::debug!("sync: the old copy of {path} was not removed yet: {e}");
                }
                Ok((etag, target))
            }
            _ => Ok((self.put_form(path, data, cond, target)?, target)),
        }
    }

    /// Deletes the copy in a given form.
    pub fn delete(&self, path: &str, enc: bool, if_match: Option<&str>) -> Result<()> {
        if enc {
            self.remote.delete(&remote_name(path), if_match)
        } else {
            self.remote.delete(path, if_match)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_path_stored_in_both_forms_keeps_the_one_the_rules_ask_for() {
        let paths = EncryptedPaths::new(&["pages/secret/".to_string()]);
        let mut m = RemoteMaps::default();
        m.add("pages/a.md", "e1".into(), 1);
        m.add("pages/secret/b.md", "e2".into(), 2);
        m.add("pages/secret/b.md.enc", "e3".into(), 3);
        m.add("pages/c.md", "e4".into(), 4);
        m.add("pages/c.md.enc", "e5".into(), 5);
        m.add("pages/d.md.enc", "e6".into(), 6);
        let (found, stale) = m.resolve(&paths);
        assert!(!found["pages/a.md"].enc);
        assert_eq!(found["pages/secret/b.md"].etag, "e3");
        assert!(found["pages/secret/b.md"].enc);
        assert_eq!(found["pages/c.md"].etag, "e4");
        assert!(found["pages/d.md"].enc);
        assert_eq!(stale.len(), 2);
        assert!(
            stale
                .iter()
                .any(|s| s.path == "pages/secret/b.md" && !s.enc)
        );
        assert!(stale.iter().any(|s| s.path == "pages/c.md" && s.enc));
    }
}
