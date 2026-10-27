//! Version history of page files (VH-1..VH-3): the last states of every file the app has written or seen change,
//! kept outside the graph in `<data dir>/versions/<hash of the path>/<unix ms>.md`.
//!
//! A version is recorded where the merge-base copy in `history/` is updated: after every write by the app and
//! whenever a change from outside (sync, another editor) is noticed. The content before the change is recorded
//! too, so the first edit of a file also keeps the original. Writes within a minute of each other replace each
//! other (only those written by the app itself: a change from outside, or the text a restore replaces, is never
//! overwritten), and old versions thin out: everything for 10 minutes, then one per 10 minutes for 3 hours, one per
//! hour for 2 days, one per day for 30 days. The oldest version is kept for 30 days.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::{Error, Graph, Inner, Result, check_rel, hash_bytes};

const BURST_MS: i64 = 60_000;
const MIN: i64 = 60_000;
const HOUR: i64 = 60 * MIN;
const DAY: i64 = 24 * HOUR;

/// One stored state of a page file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageVersion {
    /// Opaque id: the unix time in milliseconds when it was recorded.
    pub id: String,
    /// The same as a number.
    pub at_ms: i64,
    /// Size of the file at that time.
    pub bytes: u64,
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
}

/// `<id>.md`, or `<id>-w.md` for a state the app wrote itself (the one a following burst of edits replaces).
fn version_file(dir: &Path, id: i64, written: bool) -> PathBuf {
    dir.join(format!("{id}{}.md", if written { "-w" } else { "" }))
}

/// `(id, written by the app)`, newest first.
fn entries_in(dir: &Path) -> Vec<(i64, bool)> {
    let mut ids: Vec<(i64, bool)> = std::fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .filter_map(|e| {
                    let name = e.file_name();
                    let stem = name.to_str()?.strip_suffix(".md")?;
                    match stem.strip_suffix("-w") {
                        Some(n) => Some((n.parse::<i64>().ok()?, true)),
                        None => Some((stem.parse::<i64>().ok()?, false)),
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    ids.sort_unstable_by(|a, b| b.cmp(a));
    ids
}

fn ids_in(dir: &Path) -> Vec<i64> {
    entries_in(dir).into_iter().map(|e| e.0).collect()
}

fn find_version(dir: &Path, id: i64) -> Option<PathBuf> {
    [false, true]
        .into_iter()
        .map(|w| version_file(dir, id, w))
        .find(|p| p.exists())
}

/// Which versions to delete, given all ids newest first: the newest of each 10 minutes for 3 hours, of each hour
/// for 2 days, of each day for 30 days; nothing older.
pub(crate) fn expired(newest_first: &[i64], now: i64) -> Vec<i64> {
    let mut seen: BTreeSet<(u8, i64)> = BTreeSet::new();
    let mut out = Vec::new();
    let last = newest_first.len().saturating_sub(1);
    for (i, &id) in newest_first.iter().enumerate() {
        let age = now - id;
        let key = if i == 0 || age < 10 * MIN {
            continue; // the newest, and everything of the last 10 minutes (bursts are one version already)
        } else if i == last && age < 30 * DAY {
            continue; // the oldest: the original of the work that is still being done on the page
        } else if age < 3 * HOUR {
            (1, id.div_euclid(10 * MIN))
        } else if age < 2 * DAY {
            (2, id.div_euclid(HOUR))
        } else if age < 30 * DAY {
            (3, id.div_euclid(DAY))
        } else {
            out.push(id);
            continue;
        };
        if !seen.insert(key) {
            out.push(id);
        }
    }
    out
}

impl Inner {
    fn versions_root(&self) -> PathBuf {
        self.data_dir.join("versions")
    }

    fn versions_dir(&self, rel: &str) -> PathBuf {
        self.versions_root().join(&hash_bytes(rel.as_bytes())[..32])
    }

    /// Records a change of `rel` from `old` to `new` (see the module comment). Failures are ignored: it is a
    /// convenience, never a reason to fail a write.
    pub(crate) fn record_versions(&self, rel: &str, old: &[u8], new: &[u8], app_write: bool) {
        self.record_versions_at(rel, old, new, app_write, now_ms());
    }

    pub(crate) fn record_versions_at(
        &self,
        rel: &str,
        old: &[u8],
        new: &[u8],
        app_write: bool,
        now: i64,
    ) {
        if old == new {
            return;
        }
        let dir = self.versions_dir(rel);
        if std::fs::create_dir_all(&dir).is_err() {
            return;
        }
        let mut entries = entries_in(&dir);
        let newest_is_old = entries
            .first()
            .and_then(|&(id, w)| std::fs::read(version_file(&dir, id, w)).ok())
            .is_some_and(|c| c == old);
        let mut next = now;
        if !newest_is_old {
            // the state before the change was never recorded: the original, or what an outside change replaced
            if std::fs::write(version_file(&dir, next, false), old).is_ok() {
                entries.insert(0, (next, false));
                next += 1;
            }
        } else if let Some(&(newest, true)) = entries.first()
            && app_write
            && now - newest < BURST_MS
        {
            // a burst of edits: the newest version follows along instead of piling up
            let _ = std::fs::remove_file(version_file(&dir, newest, true));
            entries.remove(0);
        }
        if std::fs::write(version_file(&dir, next, app_write), new).is_ok() {
            entries.insert(0, (next, app_write));
        }
        let ids: Vec<i64> = entries.iter().map(|e| e.0).collect();
        for id in expired(&ids, now) {
            if let Some((_, w)) = entries.iter().find(|e| e.0 == id) {
                let _ = std::fs::remove_file(version_file(&dir, id, *w));
            }
        }
    }

    /// Removes the history of files that have not changed for over 35 days (deleted or renamed away).
    pub(crate) fn prune_versions(&self) {
        let now = now_ms();
        let Ok(rd) = std::fs::read_dir(self.versions_root()) else {
            return;
        };
        for e in rd.flatten() {
            let newest = ids_in(&e.path()).first().copied();
            if newest.is_none_or(|n| now - n > 35 * DAY) {
                let _ = std::fs::remove_dir_all(e.path());
            }
        }
    }

    /// Makes the newest version permanent: the next change adds a version instead of replacing it. Used before the
    /// app replaces a file on purpose (a restore), so the text that goes away stays in the history.
    pub(crate) fn pin_newest_version(&self, rel: &str) {
        let dir = self.versions_dir(rel);
        if let Some(&(id, true)) = entries_in(&dir).first() {
            let _ = std::fs::rename(version_file(&dir, id, true), version_file(&dir, id, false));
        }
    }

    /// The history of a file follows it when the page is renamed or moved.
    pub(crate) fn move_versions(&self, from: &str, to: &str) {
        let (a, b) = (self.versions_dir(from), self.versions_dir(to));
        if a.exists() && !b.exists() {
            let _ = std::fs::rename(a, b);
        }
    }
}

impl Graph {
    /// The recorded versions of a page file, newest first.
    pub fn page_versions(&self, rel: &str) -> Result<Vec<PageVersion>> {
        check_rel(rel)?;
        let dir = self.lock().versions_dir(rel);
        Ok(ids_in(&dir)
            .into_iter()
            .map(|id| PageVersion {
                id: id.to_string(),
                at_ms: id,
                bytes: find_version(&dir, id)
                    .and_then(|p| std::fs::metadata(p).ok())
                    .map_or(0, |m| m.len()),
            })
            .collect())
    }

    /// The text of one version.
    pub fn read_page_version(&self, rel: &str, id: &str) -> Result<String> {
        check_rel(rel)?;
        if id.is_empty() || !id.bytes().all(|b| b.is_ascii_digit()) {
            return Err(Error::NotFound(format!("version `{id}`")));
        }
        let dir = self.lock().versions_dir(rel);
        let missing = || Error::NotFound(format!("version `{id}` of {rel}"));
        let path = id
            .parse::<i64>()
            .ok()
            .and_then(|n| find_version(&dir, n))
            .ok_or_else(missing)?;
        std::fs::read_to_string(path).map_err(|_| missing())
    }

    /// Puts a version back as the current file (guarded like any write). The text it replaces becomes a version.
    pub fn restore_page_version(&self, rel: &str, id: &str) -> Result<()> {
        let text = self.read_page_version(rel, id)?;
        let current = match self.read_page(rel) {
            Ok(p) => Some(p.hash),
            Err(Error::NotFound(_)) => None,
            Err(e) => return Err(e),
        };
        self.lock().pin_newest_version(rel);
        self.write_page(rel, &text, current.as_deref())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thinning_keeps_the_newest_and_one_per_bucket() {
        let now = 100 * DAY;
        // a version every 5 minutes for 6 hours, newest first
        let ids: Vec<i64> = (0..72).map(|i| now - i * 5 * MIN).collect();
        let gone = expired(&ids, now);
        let kept: Vec<i64> = ids.iter().copied().filter(|i| !gone.contains(i)).collect();
        assert_eq!(kept[0], now);
        // 3 hours at one per 10 minutes, then hourly
        assert!((20..=28).contains(&kept.len()), "{}", kept.len());
        // the oldest of them is still there
        assert_eq!(*kept.last().unwrap(), now - 71 * 5 * MIN);
        // nothing older than 30 days survives
        assert_eq!(expired(&[now, now - 31 * DAY], now), vec![now - 31 * DAY]);
        // the newest is kept even when it is old
        assert!(expired(&[now - 40 * DAY], now).is_empty());
    }
}
