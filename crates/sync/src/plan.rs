use std::collections::{BTreeMap, BTreeSet};

use crate::local::LocalFile;
use crate::remote::norm_etag;
use crate::state::FileState;

/// What to do with one path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Upload the local file. `etag` is the version on the server it replaces; `None` for a new file.
    Push { path: String, etag: Option<String> },
    /// Download the file over the local one (or as a new one).
    Pull { path: String },
    /// Delete the file on the server (it was deleted here, and it did not change there).
    DeleteRemote { path: String, etag: String },
    /// Delete the local file, to the trash (it was deleted there, and it did not change here).
    DeleteLocal { path: String },
    /// Both sides have the file and both differ from the last sync (or there was no last sync): download it and
    /// see whether the contents are the same; if not, it is a conflict.
    Reconcile { path: String },
    /// The file is the same on both sides, but the server holds it in another form than the rules ask for (as it is
    /// or encrypted, EN-16): store it in the form the rules ask for, then remove the old copy.
    Recode { path: String, etag: String },
    /// The file is gone on both sides: drop what is remembered about it.
    Forget { path: String },
}

impl Action {
    /// The path.
    pub fn path(&self) -> &str {
        match self {
            Action::Push { path, .. }
            | Action::Pull { path }
            | Action::DeleteRemote { path, .. }
            | Action::DeleteLocal { path }
            | Action::Reconcile { path }
            | Action::Recode { path, .. }
            | Action::Forget { path } => path,
        }
    }
}

/// What each side has now, and what both had at the last sync. Pure: no I/O.
///
/// A side that changed wins over a side that deleted (an edit is never lost to a deletion); a file that changed on
/// both sides is a [`Action::Reconcile`].
pub fn plan(
    local: &BTreeMap<String, LocalFile>,
    remote: &BTreeMap<String, (String, u64)>,
    state: &BTreeMap<String, FileState>,
) -> Vec<Action> {
    let mut paths: BTreeSet<&String> = local.keys().collect();
    paths.extend(remote.keys());
    paths.extend(state.keys());
    let mut out = Vec::new();
    for p in paths {
        let path = p.clone();
        let l = local.get(p);
        let r = remote.get(p).map(|(etag, _)| norm_etag(etag));
        let s = state.get(p);
        let action = match (l, r, s) {
            (None, None, Some(_)) => Some(Action::Forget { path }),
            (None, None, None) => None,
            (Some(_), None, None) => Some(Action::Push { path, etag: None }),
            (None, Some(_), None) => Some(Action::Pull { path }),
            (Some(_), Some(_), None) => Some(Action::Reconcile { path }),
            (Some(l), Some(r), Some(s)) => {
                let (lc, rc) = (l.hash != s.hash, r != norm_etag(&s.etag));
                match (lc, rc) {
                    (false, false) => None,
                    (true, false) => Some(Action::Push {
                        path,
                        etag: Some(s.etag.clone()),
                    }),
                    (false, true) => Some(Action::Pull { path }),
                    (true, true) => Some(Action::Reconcile { path }),
                }
            }
            // deleted on the server
            (Some(l), None, Some(s)) => Some(if l.hash == s.hash {
                Action::DeleteLocal { path }
            } else {
                Action::Push { path, etag: None }
            }),
            // deleted here
            (None, Some(r), Some(s)) => Some(if r == norm_etag(&s.etag) {
                Action::DeleteRemote {
                    path,
                    etag: s.etag.clone(),
                }
            } else {
                Action::Pull { path }
            }),
        };
        out.extend(action);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn l(hash: &str) -> LocalFile {
        LocalFile {
            hash: hash.into(),
            size: 1,
            mtime_ms: 0,
        }
    }
    fn s(hash: &str, etag: &str) -> FileState {
        FileState {
            hash: hash.into(),
            etag: etag.into(),
            size: 1,
            mtime_ms: 0,
            enc: false,
            name: None,
        }
    }

    /// One path in each situation: (local hash, remote etag, state hash+etag) -> the action.
    fn one(
        local: Option<&str>,
        remote: Option<&str>,
        state: Option<(&str, &str)>,
    ) -> Option<Action> {
        let lm: BTreeMap<_, _> = local.map(|h| ("p".to_string(), l(h))).into_iter().collect();
        let rm: BTreeMap<_, _> = remote
            .map(|e| ("p".to_string(), (e.to_string(), 1)))
            .into_iter()
            .collect();
        let sm: BTreeMap<_, _> = state
            .map(|(h, e)| ("p".to_string(), s(h, e)))
            .into_iter()
            .collect();
        plan(&lm, &rm, &sm).into_iter().next()
    }
    fn p() -> String {
        "p".into()
    }

    #[test]
    fn a_path_is_decided_from_both_sides_and_the_last_sync() {
        // never synced
        assert_eq!(
            one(Some("a"), None, None),
            Some(Action::Push {
                path: p(),
                etag: None
            })
        );
        assert_eq!(
            one(None, Some("e1"), None),
            Some(Action::Pull { path: p() })
        );
        assert_eq!(
            one(Some("a"), Some("e1"), None),
            Some(Action::Reconcile { path: p() })
        );
        assert_eq!(one(None, None, None), None);
        // synced as (a, e1)
        let st = Some(("a", "e1"));
        assert_eq!(one(Some("a"), Some("e1"), st), None);
        assert_eq!(
            one(Some("a"), Some("\"e1\""), st),
            None,
            "quotes do not matter"
        );
        assert_eq!(
            one(Some("b"), Some("e1"), st),
            Some(Action::Push {
                path: p(),
                etag: Some("e1".into())
            })
        );
        assert_eq!(
            one(Some("a"), Some("e2"), st),
            Some(Action::Pull { path: p() })
        );
        assert_eq!(
            one(Some("b"), Some("e2"), st),
            Some(Action::Reconcile { path: p() })
        );
        // deleted on the server: the unchanged local file goes, an edited one is kept and uploaded again
        assert_eq!(
            one(Some("a"), None, st),
            Some(Action::DeleteLocal { path: p() })
        );
        assert_eq!(
            one(Some("b"), None, st),
            Some(Action::Push {
                path: p(),
                etag: None
            })
        );
        // deleted here: the unchanged remote file goes, an edited one comes back
        assert_eq!(
            one(None, Some("e1"), st),
            Some(Action::DeleteRemote {
                path: p(),
                etag: "e1".into()
            })
        );
        assert_eq!(one(None, Some("e2"), st), Some(Action::Pull { path: p() }));
        // gone on both sides
        assert_eq!(one(None, None, st), Some(Action::Forget { path: p() }));
    }
}
