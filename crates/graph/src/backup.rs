//! Snapshots of a graph (BK-1..BK-5): one zip per snapshot in a folder outside the graph.
//!
//! A snapshot holds every file of the graph folder except `.notatnik/tmp/` (temp files of atomic writes), so
//! restoring it gives a folder that opens as a graph; the index is rebuilt from the files. History copies, the
//! trash and secrets live outside the graph folder and are not part of a snapshot.
//!
//! The file name is `notatnik-<graph id>-<YYYYMMDD>-<HHMMSS>.zip`. The zip comment holds a fingerprint of the
//! files (path, size, modification time) so an unchanged graph is not saved again.

use std::collections::BTreeSet;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use chrono::{Datelike, NaiveDate, NaiveDateTime, Timelike};

use crate::{Error, Graph, Result};

/// Snapshots kept: the newest of each of the last 7 days, 4 weeks and 6 months that have one.
const KEEP_DAYS: usize = 7;
const KEEP_WEEKS: usize = 4;
const KEEP_MONTHS: usize = 6;
const COMMENT_PREFIX: &str = "notatnik-backup v1 ";

/// One snapshot file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupFile {
    /// File name, e.g. `notatnik-gab12cd-20261010-093000.zip`.
    pub name: String,
    /// Full path.
    pub path: PathBuf,
    /// When it was taken (local time of the device).
    pub created: NaiveDateTime,
    /// Size of the zip in bytes.
    pub bytes: u64,
}

fn file_name(graph_id: &str, at: NaiveDateTime) -> String {
    format!("notatnik-{graph_id}-{}.zip", at.format("%Y%m%d-%H%M%S"))
}

fn parse_name(graph_id: &str, name: &str) -> Option<NaiveDateTime> {
    let stamp = name
        .strip_prefix("notatnik-")?
        .strip_prefix(graph_id)?
        .strip_prefix('-')?
        .strip_suffix(".zip")?;
    NaiveDateTime::parse_from_str(stamp, "%Y%m%d-%H%M%S").ok()
}

/// A snapshot name is a plain file name of this module's form: never a path.
fn check_name(name: &str) -> Result<()> {
    let ok = name.starts_with("notatnik-")
        && name.ends_with(".zip")
        && !name.contains(['/', '\\'])
        && !name.contains("..");
    if ok {
        Ok(())
    } else {
        Err(Error::InvalidPath(format!("`{name}` is not a backup name")))
    }
}

/// The snapshots of one graph in `dir`, newest first. A missing folder has none.
pub fn list_backups(dir: &Path, graph_id: &str) -> Vec<BackupFile> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<BackupFile> = rd
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_str()?.to_string();
            let created = parse_name(graph_id, &name)?;
            let bytes = e.metadata().ok()?.len();
            Some(BackupFile {
                name,
                path: e.path(),
                created,
                bytes,
            })
        })
        .collect();
    out.sort_by(|a, b| b.created.cmp(&a.created).then(b.name.cmp(&a.name)));
    out
}

/// Which snapshots to delete: everything but the newest of each of the last days, weeks and months.
/// `newest_first` must be sorted that way.
pub fn expired(newest_first: &[BackupFile]) -> Vec<&BackupFile> {
    let mut days: BTreeSet<NaiveDate> = BTreeSet::new();
    let mut weeks: BTreeSet<(i32, u32)> = BTreeSet::new();
    let mut months: BTreeSet<(i32, u32)> = BTreeSet::new();
    let mut out = Vec::new();
    for b in newest_first {
        let d = b.created.date();
        let w = d.iso_week();
        let mut keep = false;
        if days.len() < KEEP_DAYS && days.insert(d) {
            keep = true;
        }
        if weeks.len() < KEEP_WEEKS && weeks.insert((w.year(), w.week())) {
            keep = true;
        }
        if months.len() < KEEP_MONTHS && months.insert((d.year(), d.month())) {
            keep = true;
        }
        if !keep {
            out.push(b);
        }
    }
    out
}

/// What is in the graph folder that a snapshot saves: `(relative path with /, absolute path)`, sorted.
fn collect(root: &Path, skip: &Path) -> Vec<(String, PathBuf)> {
    fn walk(dir: &Path, rel: &str, skip: &Path, out: &mut Vec<(String, PathBuf)>) {
        let Ok(rd) = std::fs::read_dir(dir) else {
            return;
        };
        for e in rd.flatten() {
            let Some(name) = e.file_name().to_str().map(str::to_string) else {
                continue;
            };
            let path = e.path();
            let rel = if rel.is_empty() {
                name.clone()
            } else {
                format!("{rel}/{name}")
            };
            let Ok(ty) = e.file_type() else { continue };
            if ty.is_dir() {
                // temp files of atomic writes, and the backup folder itself if someone put it here
                if rel == ".notatnik/tmp" || path == skip {
                    continue;
                }
                walk(&path, &rel, skip, out);
            } else if ty.is_file() {
                out.push((rel, path));
            }
        }
    }
    let mut out = Vec::new();
    walk(root, "", skip, &mut out);
    out.sort();
    out
}

fn fingerprint(files: &[(String, PathBuf)]) -> String {
    let mut h = blake3::Hasher::new();
    for (rel, abs) in files {
        let (len, mtime) = std::fs::metadata(abs).map_or((0, 0), |m| {
            let t = m
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map_or(0, |d| d.as_nanos());
            (m.len(), t)
        });
        h.update(format!("{rel}\0{len}\0{mtime}\n").as_bytes());
    }
    h.finalize().to_hex().to_string()
}

fn zip_err(e: zip::result::ZipError) -> Error {
    Error::Io(std::io::Error::other(e.to_string()))
}

/// The fingerprint stored in a snapshot (None for a file that is not one of ours).
fn stored_fingerprint(path: &Path) -> Option<String> {
    let f = std::fs::File::open(path).ok()?;
    let z = zip::ZipArchive::new(f).ok()?;
    let c = String::from_utf8_lossy(z.comment()).into_owned();
    c.strip_prefix(COMMENT_PREFIX).map(str::to_string)
}

/// A restore target must be a folder that does not exist yet or is empty.
fn check_target(target: &Path) -> Result<()> {
    match std::fs::read_dir(target) {
        Ok(mut rd) => {
            if rd.next().is_some() {
                return Err(Error::InvalidPath(format!(
                    "{} is not empty",
                    target.display()
                )));
            }
            Ok(())
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
    }
}

/// Unpacks a snapshot into `target` (a new or empty folder). Returns the number of files.
/// The result opens as a graph. Entries with unsafe names are skipped.
pub fn restore_backup(zip_path: &Path, target: &Path) -> Result<usize> {
    check_target(target)?;
    let f = std::fs::File::open(zip_path)?;
    let mut z = zip::ZipArchive::new(f).map_err(zip_err)?;
    std::fs::create_dir_all(target)?;
    let mut n = 0;
    for i in 0..z.len() {
        let mut entry = z.by_index(i).map_err(zip_err)?;
        let Some(rel) = entry.enclosed_name() else {
            continue;
        };
        if entry.is_dir() {
            continue;
        }
        let dest = target.join(rel);
        if let Some(d) = dest.parent() {
            std::fs::create_dir_all(d)?;
        }
        let mut out = std::fs::File::create(&dest)?;
        std::io::copy(&mut entry, &mut out)?;
        n += 1;
    }
    Ok(n)
}

impl Graph {
    /// Takes a snapshot into `dir` (created if needed) and removes the expired ones.
    ///
    /// With `force == false` nothing happens (`None`) when the newest snapshot is younger than `min_age` or the
    /// files are the same as in it.
    pub fn backup_now(
        &self,
        dir: &Path,
        now: NaiveDateTime,
        force: bool,
        min_age: Option<chrono::Duration>,
    ) -> Result<Option<BackupFile>> {
        self.snapshot(dir, now, force, min_age, None)
    }

    /// [`Graph::backup_now`]; the snapshot named `protect` is never removed by the retention.
    fn snapshot(
        &self,
        dir: &Path,
        now: NaiveDateTime,
        force: bool,
        min_age: Option<chrono::Duration>,
        protect: Option<&str>,
    ) -> Result<Option<BackupFile>> {
        let root = self.root().to_path_buf();
        let id = self.id();
        if dir.starts_with(&root) {
            return Err(Error::InvalidPath(
                "the backup folder must be outside the graph folder".into(),
            ));
        }
        let existing = list_backups(dir, &id);
        let files = collect(&root, dir);
        let fp = fingerprint(&files);
        if !force {
            if let Some(newest) = existing.first() {
                if min_age.is_some_and(|a| now.signed_duration_since(newest.created) < a) {
                    return Ok(None);
                }
                if stored_fingerprint(&newest.path).as_deref() == Some(fp.as_str()) {
                    return Ok(None);
                }
            }
        }
        std::fs::create_dir_all(dir)?;
        // two snapshots in one second (a forced one right after another): the later one waits for a free name
        let mut at = now.with_nanosecond(0).unwrap_or(now);
        while dir.join(file_name(&id, at)).exists() {
            at += chrono::Duration::seconds(1);
        }
        let name = file_name(&id, at);
        let dest = dir.join(&name);
        let partial = dir.join(format!("{name}.partial"));
        let written = (|| -> Result<()> {
            let f = std::fs::File::create(&partial)?;
            let mut z = zip::ZipWriter::new(std::io::BufWriter::new(f));
            z.set_comment(format!("{COMMENT_PREFIX}{fp}"));
            let opts = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated);
            for (rel, abs) in &files {
                // a file that vanished or is unreadable now is skipped: the next snapshot has it
                let Ok(mut src) = std::fs::File::open(abs) else {
                    continue;
                };
                z.start_file(rel.as_str(), opts).map_err(zip_err)?;
                let mut buf = Vec::new();
                if src.read_to_end(&mut buf).is_ok() {
                    z.write_all(&buf)?;
                }
            }
            let mut w = z.finish().map_err(zip_err)?;
            w.flush()?;
            w.get_ref().sync_all()?;
            Ok(())
        })();
        if let Err(e) = written {
            let _ = std::fs::remove_file(&partial);
            return Err(e);
        }
        std::fs::rename(&partial, &dest)?;
        let bytes = std::fs::metadata(&dest).map_or(0, |m| m.len());
        let made = BackupFile {
            name,
            path: dest,
            created: at,
            bytes,
        };
        let all = list_backups(dir, &id);
        for old in expired(&all) {
            if old.name != made.name && Some(old.name.as_str()) != protect {
                let _ = std::fs::remove_file(&old.path);
            }
        }
        Ok(Some(made))
    }

    /// Puts one page from a snapshot back. The snapshot is read first, then a new snapshot of the graph as it is
    /// now is taken (so the version that gets replaced is not lost), then the file is replaced atomically.
    pub fn restore_page_from_backup(
        &self,
        dir: &Path,
        name: &str,
        rel: &str,
        now: NaiveDateTime,
    ) -> Result<()> {
        check_name(name)?;
        crate::check_rel(rel)?;
        let f = std::fs::File::open(dir.join(name))?;
        let mut z = zip::ZipArchive::new(f).map_err(zip_err)?;
        let mut text = String::new();
        {
            let mut entry = z
                .by_name(rel)
                .map_err(|_| Error::NotFound(format!("{rel} in {name}")))?;
            entry.read_to_string(&mut text)?;
        }
        self.snapshot(dir, now, true, None, Some(name))?;
        let current = match self.read_page(rel) {
            Ok(p) => Some(p.hash),
            Err(Error::NotFound(_)) => None,
            Err(e) => return Err(e),
        };
        self.write_page(rel, &text, current.as_deref())?;
        Ok(())
    }

    /// Unpacks a snapshot of this graph from `dir` into a new or empty folder.
    pub fn restore_backup_to(&self, dir: &Path, name: &str, target: &Path) -> Result<usize> {
        check_name(name)?;
        let root = self.root().to_path_buf();
        if target.starts_with(&root) {
            return Err(Error::InvalidPath(
                "restore into a folder outside the graph folder".into(),
            ));
        }
        restore_backup(&dir.join(name), target)
    }

    /// The snapshots of this graph in `dir`, newest first.
    pub fn backups(&self, dir: &Path) -> Vec<BackupFile> {
        list_backups(dir, &self.id())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(s: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S").unwrap()
    }

    fn fake(id: &str, s: &str) -> BackupFile {
        let created = at(s);
        let name = file_name(id, created);
        BackupFile {
            path: PathBuf::from(&name),
            name,
            created,
            bytes: 1,
        }
    }

    #[test]
    fn names_round_trip_and_belong_to_one_graph() {
        let n = file_name("gab12cd", at("2026-10-10 09:30:05"));
        assert_eq!(n, "notatnik-gab12cd-20261010-093005.zip");
        assert_eq!(parse_name("gab12cd", &n), Some(at("2026-10-10 09:30:05")));
        assert_eq!(parse_name("gother", &n), None);
        assert_eq!(parse_name("gab12cd", "notatnik-gab12cd-x.zip"), None);
        assert!(check_name(&n).is_ok());
        assert!(check_name("../notatnik-g-20261010-093005.zip").is_err());
        assert!(check_name("notes.zip").is_err());
    }

    #[test]
    fn retention_keeps_days_weeks_and_months() {
        // one snapshot per day for 120 days, newest first
        let start = at("2026-10-10 12:00:00");
        let all: Vec<BackupFile> = (0..120)
            .map(|d| {
                let t = start - chrono::Duration::days(d);
                fake("g1", &t.format("%Y-%m-%d %H:%M:%S").to_string())
            })
            .collect();
        let gone = expired(&all);
        let kept = all.len() - gone.len();
        // 7 days, plus the newest of up to 4 weeks and 6 months (some coincide with the days)
        assert!((10..=17).contains(&kept), "kept {kept}");
        // the newest and the last 7 days are always kept
        assert!(
            gone.iter()
                .all(|b| b.created < start - chrono::Duration::days(6))
        );
        // two on one day: only the newest stays
        let two = vec![
            fake("g1", "2026-10-10 18:00:00"),
            fake("g1", "2026-10-10 08:00:00"),
        ];
        let gone = expired(&two);
        assert_eq!(gone.len(), 1);
        assert_eq!(gone[0].created, at("2026-10-10 08:00:00"));
    }
}
