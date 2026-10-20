//! Snapshots of a graph (BK-1..BK-5): one zip per snapshot in a folder outside the graph.
//!
//! A snapshot holds every file of the graph folder except `.notatnik/tmp/` (temp files of atomic writes), so
//! restoring it gives a folder that opens as a graph; the index is rebuilt from the files. History copies, the
//! trash and secrets live outside the graph folder and are not part of a snapshot.
//!
//! The file name is `notatnik-<graph id>-<YYYYMMDD>-<HHMMSS>.zip`. The zip comment holds a fingerprint of the
//! files (path, size, modification time) so an unchanged graph is not saved again.
//!
//! Besides the graph's files a snapshot can carry the device's own configuration (BK-8): files outside the graph
//! folder, stored under `.notatnik-device/` (see [`BackupOptions`]). Unpacking a snapshot into a folder leaves them
//! out, because that folder is a graph and holds nothing that is not meant to be synced (ST-2). Entries are
//! compressed (deflate) or only stored (BK-7); both are ordinary zip files.
//!
//! Files that the graph marks as encrypted (`[encryption] paths`, EN-11) are never stored plain in a snapshot
//! (EN-23): each is an entry `<path>.enc` holding a sealed file (EN-5) for the graph's vault, so writing needs no
//! password; reading needs the graph's password or recovery key. A snapshot that is sealed as a whole is not
//! sealed again entry by entry.
//!
//! A second folder keeps copies of the snapshots (BK-9, [`sync_mirror`]).

use std::collections::BTreeSet;
use std::io::{Read, Seek, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::{Datelike, NaiveDate, NaiveDateTime, Timelike};
use notatnik_crypt::{Keys, Vault};

use crate::{Error, Graph, Result};

/// Snapshots kept: the newest of each of the last 7 days, 4 weeks and 6 months that have one.
const KEEP_DAYS: usize = 7;
const KEEP_WEEKS: usize = 4;
const KEEP_MONTHS: usize = 6;
/// Snapshots taken before a bulk change (`-before` in the name) are not part of the day/week/month scheme: the
/// newest ones are kept.
const KEEP_BEFORE: usize = 10;
const COMMENT_PREFIX: &str = "notatnik-backup v1 ";
/// Folder inside a snapshot for the device files (BK-8).
pub const DEVICE_DIR: &str = ".notatnik-device";

/// A file outside the graph folder that goes into every snapshot (BK-8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Extra {
    /// Name inside the snapshot, below [`DEVICE_DIR`], e.g. `settings.toml`.
    pub name: String,
    /// The file. A missing file is left out.
    pub path: PathBuf,
    /// A change of this file counts as "something changed": it makes a new snapshot due. Files that change all the
    /// time (the window state) are not tracked; they are saved with the next snapshot that is due anyway.
    pub tracked: bool,
}

/// How a snapshot is written and read.
#[derive(Debug, Clone)]
pub struct BackupOptions {
    /// Deflate the entries (BK-7). `false` only stores them: bigger files, but they stay readable in place and suit
    /// a target that compresses or deduplicates by itself.
    pub compress: bool,
    /// Device files saved with the graph.
    pub extras: Vec<Extra>,
    /// Seal new snapshots to this vault's public key (EN-5): no password is needed to write them, and the file
    /// is `...zip.enc`. Never falls back to a plain zip: the caller decides whether to write at all.
    pub seal_to: Option<Vault>,
    /// The unlocked vault, needed to read an encrypted snapshot (compare, restore) and used to read a new one
    /// back completely. `None` while the backups are locked.
    pub keys: Option<Arc<Keys>>,
    /// The unlocked key of the graph's own vault (EN-13), needed to read the files that a snapshot holds sealed
    /// (EN-23). Writing them needs no key.
    pub graph_keys: Option<Arc<Keys>>,
}

impl Default for BackupOptions {
    fn default() -> Self {
        BackupOptions {
            compress: true,
            extras: Vec::new(),
            seal_to: None,
            keys: None,
            graph_keys: None,
        }
    }
}

/// One snapshot file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupFile {
    /// File name, e.g. `notatnik-gab12cd-20261010-093000.zip`.
    pub name: String,
    /// Full path.
    pub path: PathBuf,
    /// When it was taken (local time of the device).
    pub created: NaiveDateTime,
    /// Taken automatically before a bulk change (rename, delete, restore), not on the schedule.
    pub before: bool,
    /// Size of the file in bytes.
    pub bytes: u64,
    /// Sealed to a vault (`.zip.enc`): reading it needs the password or the recovery key.
    pub encrypted: bool,
}

fn file_name(graph_id: &str, at: NaiveDateTime, before: bool, encrypted: bool) -> String {
    let tag = if before { "-before" } else { "" };
    let ext = if encrypted { "zip.enc" } else { "zip" };
    format!(
        "notatnik-{graph_id}-{}{tag}.{ext}",
        at.format("%Y%m%d-%H%M%S")
    )
}

fn parse_name(graph_id: &str, name: &str) -> Option<(NaiveDateTime, bool, bool)> {
    let rest = name
        .strip_prefix("notatnik-")?
        .strip_prefix(graph_id)?
        .strip_prefix('-')?;
    let (stamp, encrypted) = match rest.strip_suffix(".zip.enc") {
        Some(s) => (s, true),
        None => (rest.strip_suffix(".zip")?, false),
    };
    let (stamp, before) = match stamp.strip_suffix("-before") {
        Some(s) => (s, true),
        None => (stamp, false),
    };
    let at = NaiveDateTime::parse_from_str(stamp, "%Y%m%d-%H%M%S").ok()?;
    Some((at, before, encrypted))
}

/// A snapshot name is a plain file name of this module's form: never a path.
fn check_name(name: &str) -> Result<()> {
    let ok = name.starts_with("notatnik-")
        && (name.ends_with(".zip") || name.ends_with(".zip.enc"))
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
            let (created, before, encrypted) = parse_name(graph_id, &name)?;
            let bytes = e.metadata().ok()?.len();
            Some(BackupFile {
                name,
                path: e.path(),
                created,
                before,
                bytes,
                encrypted,
            })
        })
        .collect();
    out.sort_by(|a, b| b.created.cmp(&a.created).then(b.name.cmp(&a.name)));
    out
}

/// Which snapshots to delete: of the scheduled ones everything but the newest of each of the last days, weeks and
/// months; of the `-before` ones everything but the newest ten. `newest_first` must be sorted that way.
pub fn expired(newest_first: &[BackupFile]) -> Vec<&BackupFile> {
    let mut days: BTreeSet<NaiveDate> = BTreeSet::new();
    let mut weeks: BTreeSet<(i32, u32)> = BTreeSet::new();
    let mut months: BTreeSet<(i32, u32)> = BTreeSet::new();
    let mut out = Vec::new();
    let mut before = 0;
    for b in newest_first {
        if b.before {
            before += 1;
            if before > KEEP_BEFORE {
                out.push(b);
            }
            continue;
        }
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

/// The graph's own encryption rules and vault, for sealing the marked files of a snapshot (EN-23).
type Marks = (Vault, notatnik_core::encrypted::EncryptedPaths);

/// Start of the comment of a sealed entry; the rest is the path.
const ENTRY_COMMENT: &str = "notatnik-entry v1 ";

fn fingerprint(files: &[(String, PathBuf)], extras: &[Extra], marks: Option<&Marks>) -> String {
    let mut h = blake3::Hasher::new();
    // tracked device files by content: their modification time changes without the content changing
    for x in extras.iter().filter(|x| x.tracked) {
        if let Ok(b) = std::fs::read(&x.path) {
            h.update(format!("{DEVICE_DIR}/{}\0", x.name).as_bytes());
            h.update(blake3::hash(&b).as_bytes());
        }
    }
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
        // a file that becomes marked (or a new key) is a change, though the file is the same
        if let Some((v, _)) = marks.filter(|(_, p)| p.is_encrypted(rel)) {
            h.update(format!("sealed {}\n", v.key_id).as_bytes());
        }
    }
    h.finalize().to_hex().to_string()
}

fn zip_err(e: zip::result::ZipError) -> Error {
    Error::Io(std::io::Error::other(e.to_string()))
}

/// The fingerprint stored in a snapshot (None for a file that is not one of ours): in the zip comment, or for a
/// sealed file in the comment of its header, which can be read without a key.
fn stored_fingerprint(path: &Path) -> Option<String> {
    let f = std::fs::File::open(path).ok()?;
    let c = if path.extension().is_some_and(|e| e == "enc") {
        let h = notatnik_crypt::read_header(&mut std::io::BufReader::new(f)).ok()?;
        String::from_utf8_lossy(&h.comment).into_owned()
    } else {
        let z = zip::ZipArchive::new(f).ok()?;
        String::from_utf8_lossy(z.comment()).into_owned()
    };
    c.strip_prefix(COMMENT_PREFIX).map(str::to_string)
}

/// Opens a snapshot as a zip. A sealed one is decrypted into an unnamed temporary file first (deleted when it is
/// closed); that needs the unlocked keys.
fn open_zip(path: &Path, keys: Option<&Keys>) -> Result<zip::ZipArchive<std::fs::File>> {
    let mut f = std::fs::File::open(path)?;
    if path.extension().is_some_and(|e| e == "enc") {
        let keys = keys.ok_or_else(|| {
            Error::Locked("this snapshot is encrypted: unlock the backups first".into())
        })?;
        let mut tmp = tempfile::tempfile()?;
        {
            let mut w = std::io::BufWriter::new(&mut tmp);
            notatnik_crypt::open(keys, &mut std::io::BufReader::new(&mut f), &mut w)?;
            w.flush()?;
        }
        tmp.rewind()?;
        f = tmp;
    }
    zip::ZipArchive::new(f).map_err(zip_err)
}

/// A file of a snapshot as it was saved.
enum Saved {
    Bytes(Vec<u8>),
    /// A sealed entry and no key to open it.
    Locked,
}

/// What a zip entry holds for the graph path it names: `(path, content)`. A sealed entry (`<path>.enc`, written
/// by [`write_zip`]) is opened with `keys` and named by the path without `.enc`; without keys it is `Locked`.
/// Anything else, a user's own file called `x.enc` included, is returned as it is.
fn saved_entry(name: &str, buf: Vec<u8>, keys: Option<&Keys>) -> Result<(String, Saved)> {
    let Some(path) = name.strip_suffix(".enc") else {
        return Ok((name.to_string(), Saved::Bytes(buf)));
    };
    let ours = buf.starts_with(b"NTNKSEAL")
        && notatnik_crypt::read_header(&mut &buf[..])
            .is_ok_and(|h| h.comment == format!("{ENTRY_COMMENT}{path}").as_bytes());
    if !ours {
        return Ok((name.to_string(), Saved::Bytes(buf)));
    }
    let Some(keys) = keys else {
        return Ok((path.to_string(), Saved::Locked));
    };
    let mut plain = Vec::new();
    notatnik_crypt::open(keys, &mut &buf[..], &mut plain)?;
    Ok((path.to_string(), Saved::Bytes(plain)))
}

fn locked_error(path: &str) -> Error {
    Error::Locked(format!(
        "{path} is saved encrypted: unlock the graph's encryption first (Settings → Sync → Encryption)"
    ))
}

/// The marked files of the graph and its vault, for a snapshot that is not sealed as a whole. An error when files
/// are marked and the vault is missing: a snapshot must never hold them in plain by accident.
fn marks_of(g: &Graph, opts: &BackupOptions) -> Result<Option<Marks>> {
    if opts.seal_to.is_some() {
        return Ok(None);
    }
    let paths = notatnik_core::encrypted::EncryptedPaths::new(&g.config().encryption.paths);
    if paths.is_empty() {
        return Ok(None);
    }
    match g.read_vault()? {
        Some(v) => Ok(Some((v, paths))),
        None => Err(Error::Crypt(
            "files are marked as encrypted but .notatnik/vault.json is missing; nothing was written".into(),
        )),
    }
}

/// The content of `path` in a snapshot, whether it was saved plain or sealed (None when it has no such entry).
fn saved_file<R: Read + Seek>(
    z: &mut zip::ZipArchive<R>,
    path: &str,
    graph_keys: Option<&Keys>,
) -> Result<Option<Vec<u8>>> {
    for name in [path.to_string(), format!("{path}.enc")] {
        let Ok(mut e) = z.by_name(&name) else {
            continue;
        };
        let mut buf = Vec::new();
        e.read_to_end(&mut buf)?;
        return match saved_entry(&name, buf, graph_keys)? {
            (_, Saved::Bytes(b)) => Ok(Some(b)),
            (p, Saved::Locked) => Err(locked_error(&p)),
        };
    }
    Ok(None)
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
/// `keys` opens a sealed snapshot, `graph_keys` the files in it that are sealed for the graph's vault (EN-23): the
/// result holds them in plain, as the graph did. Without `graph_keys` a snapshot with such files is not unpacked.
pub fn restore_backup(
    zip_path: &Path,
    target: &Path,
    keys: Option<&Keys>,
    graph_keys: Option<&Keys>,
) -> Result<usize> {
    check_target(target)?;
    let mut z = open_zip(zip_path, keys)?;
    if graph_keys.is_none() {
        // refuse before anything is written
        for i in 0..z.len() {
            let mut entry = z.by_index(i).map_err(zip_err)?;
            let name = entry.name().to_string();
            if entry.is_dir() || !name.ends_with(".enc") {
                continue;
            }
            let mut buf = Vec::new();
            entry.read_to_end(&mut buf)?;
            if let (path, Saved::Locked) = saved_entry(&name, buf, None)? {
                return Err(locked_error(&path));
            }
        }
    }
    std::fs::create_dir_all(target)?;
    let mut n = 0;
    for i in 0..z.len() {
        let mut entry = z.by_index(i).map_err(zip_err)?;
        let Some(rel) = entry.enclosed_name() else {
            continue;
        };
        if entry.is_dir() || rel.starts_with(DEVICE_DIR) {
            continue;
        }
        let name = rel.to_string_lossy().replace('\\', "/");
        let (rel, content) = if name.ends_with(".enc") {
            let mut buf = Vec::new();
            entry.read_to_end(&mut buf)?;
            match saved_entry(&name, buf, graph_keys)? {
                (p, Saved::Bytes(b)) => (PathBuf::from(p), Some(b)),
                (p, Saved::Locked) => return Err(locked_error(&p)),
            }
        } else {
            (rel, None)
        };
        let dest = target.join(&rel);
        if let Some(d) = dest.parent() {
            std::fs::create_dir_all(d)?;
        }
        let mut out = std::fs::File::create(&dest)?;
        match content {
            Some(b) => out.write_all(&b)?,
            None => {
                std::io::copy(&mut entry, &mut out)?;
            }
        }
        n += 1;
    }
    Ok(n)
}

/// Writes the zip of a snapshot into `out` and returns the number of entries.
fn write_zip(
    out: &std::fs::File,
    files: &[(String, PathBuf)],
    device: &[(String, PathBuf)],
    method: zip::CompressionMethod,
    comment: &str,
    marks: Option<&Marks>,
) -> Result<usize> {
    let mut z = zip::ZipWriter::new(std::io::BufWriter::new(out));
    z.set_comment(comment);
    let zopts = zip::write::SimpleFileOptions::default().compression_method(method);
    let mut count = 0;
    for (rel, abs) in files.iter().chain(device) {
        // a file that vanished or is unreadable now is skipped: the next snapshot has it
        let Ok(buf) = std::fs::read(abs) else {
            continue;
        };
        // a marked file is saved sealed, under `<path>.enc` (EN-23)
        if let Some((vault, _)) = marks.filter(|(_, p)| p.is_encrypted(rel)) {
            let mut sealed = Vec::new();
            notatnik_crypt::seal(
                vault,
                format!("{ENTRY_COMMENT}{rel}").as_bytes(),
                &mut &buf[..],
                &mut sealed,
            )?;
            z.start_file(
                format!("{rel}.enc"),
                zopts.compression_method(zip::CompressionMethod::Stored),
            )
            .map_err(zip_err)?;
            z.write_all(&sealed)?;
            count += 1;
            continue;
        }
        // pictures are compressed already: deflating them costs time and saves nothing (ST-8)
        let opts = if crate::sniff_image(&buf).is_some() {
            zopts.compression_method(zip::CompressionMethod::Stored)
        } else {
            zopts
        };
        z.start_file(rel.as_str(), opts).map_err(zip_err)?;
        z.write_all(&buf)?;
        count += 1;
    }
    let mut w = z.finish().map_err(zip_err)?;
    w.flush()?;
    Ok(count)
}

/// Checks a sealed file that was just written: the header must read back and name this vault. With the unlocked
/// keys of that vault the whole file is decrypted as well, which also proves that the keys are the right ones.
fn check_sealed(path: &Path, vault: &Vault, keys: Option<&Keys>) -> Result<()> {
    let mut r = std::io::BufReader::new(std::fs::File::open(path)?);
    let h = notatnik_crypt::read_header(&mut r)?;
    if h.vault.key_id != vault.key_id {
        return Err(Error::Crypt("the sealed file names another vault".into()));
    }
    if let Some(k) = keys.filter(|k| k.key_id() == vault.key_id) {
        let mut r = std::io::BufReader::new(std::fs::File::open(path)?);
        notatnik_crypt::open(k, &mut r, &mut std::io::sink())?;
    }
    Ok(())
}

/// Seals every plain snapshot of the graph in `dir` to `vault` and removes the plain file afterwards (EN-6).
/// Returns how many were converted. A snapshot whose sealed twin already exists is left alone.
pub fn seal_existing(dir: &Path, graph_id: &str, vault: &Vault) -> Result<usize> {
    let mut n = 0;
    for b in list_backups(dir, graph_id)
        .into_iter()
        .filter(|b| !b.encrypted)
    {
        let dest = dir.join(format!("{}.enc", b.name));
        if dest.exists() {
            continue;
        }
        let comment = stored_fingerprint(&b.path)
            .map(|f| format!("{COMMENT_PREFIX}{f}"))
            .unwrap_or_default();
        let partial = dir.join(format!("{}.enc.partial", b.name));
        let done = (|| -> Result<()> {
            let mut w = std::io::BufWriter::new(std::fs::File::create(&partial)?);
            let src = std::fs::File::open(&b.path)?;
            notatnik_crypt::seal(
                vault,
                comment.as_bytes(),
                &mut std::io::BufReader::new(src),
                &mut w,
            )?;
            w.flush()?;
            w.get_ref().sync_all()?;
            drop(w);
            check_sealed(&partial, vault, None)
        })();
        if let Err(e) = done {
            let _ = std::fs::remove_file(&partial);
            return Err(e);
        }
        std::fs::rename(&partial, &dest)?;
        std::fs::remove_file(&b.path)?;
        n += 1;
    }
    Ok(n)
}

/// Re-reads a snapshot that was just written: it must open and hold `count` entries. A file that fails this is
/// never kept as a backup.
fn verify(path: &Path, count: usize) -> Result<()> {
    verify_zip(std::fs::File::open(path)?, count)
}

fn verify_zip<R: Read + Seek>(f: R, count: usize) -> Result<()> {
    let mut z = zip::ZipArchive::new(f).map_err(zip_err)?;
    if z.len() != count {
        return Err(Error::Io(std::io::Error::other(format!(
            "the snapshot holds {} files instead of {count}",
            z.len()
        ))));
    }
    // reading every entry to the end checks the checksums
    for i in 0..z.len() {
        let mut e = z.by_index(i).map_err(zip_err)?;
        std::io::copy(&mut e, &mut std::io::sink())?;
    }
    Ok(())
}

/// The text of one entry of a snapshot (None when it has no such entry).
pub fn read_entry(zip_path: &Path, entry: &str, keys: Option<&Keys>) -> Result<Option<String>> {
    let mut z = open_zip(zip_path, keys)?;
    let Ok(mut e) = z.by_name(entry) else {
        return Ok(None);
    };
    let mut text = String::new();
    e.read_to_string(&mut text)?;
    Ok(Some(text))
}

/// Brings the second folder up to date with the first (BK-9): snapshots of the graph that it lacks are copied (to a
/// `.partial` file first, then renamed), then its own retention removes the expired ones. Returns the number
/// copied. The second folder must already exist: a missing one (an unplugged disk, an unmounted share) is an error
/// and is never created, because a mount point that is gone would otherwise fill the system disk.
pub fn sync_mirror(primary: &Path, mirror: &Path, graph_id: &str) -> Result<usize> {
    if !mirror.is_dir() {
        return Err(Error::InvalidPath(format!(
            "the second backup folder {} is not available",
            mirror.display()
        )));
    }
    if mirror == primary {
        return Err(Error::InvalidPath(
            "the second backup folder must differ from the first".into(),
        ));
    }
    let have = list_backups(mirror, graph_id);
    let mut copied = 0;
    for b in list_backups(primary, graph_id) {
        if have.iter().any(|m| m.name == b.name && m.bytes == b.bytes) {
            continue;
        }
        let dest = mirror.join(&b.name);
        let partial = mirror.join(format!("{}.partial", b.name));
        let r = std::fs::copy(&b.path, &partial).and_then(|_| std::fs::rename(&partial, &dest));
        if let Err(e) = r {
            let _ = std::fs::remove_file(&partial);
            return Err(e.into());
        }
        copied += 1;
    }
    let all = list_backups(mirror, graph_id);
    for old in expired(&all) {
        let _ = std::fs::remove_file(&old.path);
    }
    Ok(copied)
}

/// How a file differs between a snapshot and the graph as it is now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeStatus {
    /// In both, with different content.
    Changed,
    /// Only in the snapshot (deleted or renamed since).
    OnlySnapshot,
    /// Only now (created since the snapshot).
    OnlyNow,
    /// Saved sealed (EN-23) and the graph's key is locked: not compared.
    Locked,
}

/// One file that differs (BK-10). Line counts are from the snapshot's point of view: `added` lines are in the
/// snapshot and not now (a restore would bring them back), `removed` lines are now and not in the snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileChange {
    /// Path in the graph folder, or `.notatnik-device/<name>` for a device file.
    pub path: String,
    /// How it differs.
    pub status: ChangeStatus,
    /// Not UTF-8 text: no lines are compared.
    pub binary: bool,
    /// Lines only in the snapshot.
    pub added: usize,
    /// Lines only now.
    pub removed: usize,
}

/// A line of [`diff_lines`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffOp {
    /// Same in both (context).
    Same(String),
    /// Only in the new text.
    Add(String),
    /// Only in the old text.
    Del(String),
    /// Unchanged lines left out.
    Gap,
}

/// Above this many table cells (lines × lines after the common start and end are cut off) a diff is not computed.
const MAX_DIFF_CELLS: usize = 4_000_000;

/// The lines that differ between `old` and `new` (a longest-common-subsequence diff), each change with `context`
/// unchanged lines around it and [`DiffOp::Gap`] between distant changes. `None` when the texts are too different
/// and long to compare.
pub fn diff_lines(old: &str, new: &str, context: usize) -> Option<Vec<DiffOp>> {
    let (a, b): (Vec<&str>, Vec<&str>) = (old.lines().collect(), new.lines().collect());
    let pre = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    let suf = a[pre..]
        .iter()
        .rev()
        .zip(b[pre..].iter().rev())
        .take_while(|(x, y)| x == y)
        .count();
    let (ma, mb) = (&a[pre..a.len() - suf], &b[pre..b.len() - suf]);
    if (ma.len() + 1).saturating_mul(mb.len() + 1) > MAX_DIFF_CELLS {
        return None;
    }
    let mut lcs = vec![vec![0u32; mb.len() + 1]; ma.len() + 1];
    for i in (0..ma.len()).rev() {
        for j in (0..mb.len()).rev() {
            lcs[i][j] = if ma[i] == mb[j] {
                lcs[i + 1][j + 1] + 1
            } else {
                lcs[i + 1][j].max(lcs[i][j + 1])
            };
        }
    }
    let mut ops: Vec<DiffOp> = a[..pre].iter().map(|l| DiffOp::Same((*l).into())).collect();
    let (mut i, mut j) = (0, 0);
    while i < ma.len() || j < mb.len() {
        if i < ma.len() && j < mb.len() && ma[i] == mb[j] {
            ops.push(DiffOp::Same(ma[i].into()));
            i += 1;
            j += 1;
        } else if i < ma.len() && (j == mb.len() || lcs[i + 1][j] >= lcs[i][j + 1]) {
            ops.push(DiffOp::Del(ma[i].into()));
            i += 1;
        } else {
            ops.push(DiffOp::Add(mb[j].into()));
            j += 1;
        }
    }
    ops.extend(a[a.len() - suf..].iter().map(|l| DiffOp::Same((*l).into())));
    // keep the changes and `context` lines around them
    let changed: Vec<bool> = ops.iter().map(|o| !matches!(o, DiffOp::Same(_))).collect();
    let near = |k: usize| {
        let (lo, hi) = (k.saturating_sub(context), (k + context + 1).min(ops.len()));
        changed[lo..hi].iter().any(|c| *c)
    };
    let mut out = Vec::new();
    let mut skipped = false;
    for (k, op) in ops.iter().enumerate() {
        if near(k) {
            if skipped {
                out.push(DiffOp::Gap);
                skipped = false;
            }
            out.push(op.clone());
        } else {
            skipped = true;
        }
    }
    if skipped && !out.is_empty() {
        out.push(DiffOp::Gap);
    }
    Some(out)
}

/// A file compared with the same file in a snapshot (BK-10).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileDiff {
    /// Not UTF-8 text.
    pub binary: bool,
    /// Too long and too different to compare line by line.
    pub too_large: bool,
    /// The differing lines with context (old = now, new = snapshot).
    pub lines: Vec<DiffOp>,
}

impl Graph {
    /// The current content of `path` (a graph file, or `.notatnik-device/<name>`), None when it does not exist.
    fn current_bytes(&self, path: &str, opts: &BackupOptions) -> Result<Option<Vec<u8>>> {
        let file = match path.strip_prefix(&format!("{DEVICE_DIR}/")) {
            Some(name) => match opts.extras.iter().find(|x| x.name == name) {
                Some(x) => x.path.clone(),
                None => return Ok(None),
            },
            None => {
                // any file of the graph folder (not only pages), but never a path that leaves it
                let safe = !path.is_empty()
                    && !path.contains('\\')
                    && Path::new(path)
                        .components()
                        .all(|c| matches!(c, std::path::Component::Normal(_)));
                if !safe {
                    return Err(Error::InvalidPath(format!("`{path}` is not a graph file")));
                }
                self.root().join(path)
            }
        };
        match std::fs::read(file) {
            Ok(b) => Ok(Some(b)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Every file that differs between the snapshot `name` in `dir` and the graph (and device files) as they are
    /// now, sorted by path (BK-10). Reads only; nothing changes.
    pub fn compare_backup(
        &self,
        dir: &Path,
        name: &str,
        opts: &BackupOptions,
    ) -> Result<Vec<FileChange>> {
        check_name(name)?;
        let mut z = open_zip(&dir.join(name), opts.keys.as_deref())?;
        let mut saved: std::collections::BTreeMap<String, Vec<u8>> = Default::default();
        let mut locked: BTreeSet<String> = BTreeSet::new();
        for i in 0..z.len() {
            let mut e = z.by_index(i).map_err(zip_err)?;
            let Some(rel) = e.enclosed_name() else {
                continue;
            };
            if e.is_dir() {
                continue;
            }
            let mut buf = Vec::new();
            e.read_to_end(&mut buf)?;
            let name = rel.to_string_lossy().replace('\\', "/");
            match saved_entry(&name, buf, opts.graph_keys.as_deref())? {
                (path, Saved::Bytes(b)) => {
                    saved.insert(path, b);
                }
                (path, Saved::Locked) => {
                    locked.insert(path);
                }
            }
        }
        let mut paths: BTreeSet<String> = saved.keys().cloned().collect();
        for (rel, _) in collect(self.root(), dir) {
            paths.insert(rel);
        }
        for x in &opts.extras {
            if x.path.is_file() {
                paths.insert(format!("{DEVICE_DIR}/{}", x.name));
            }
        }
        let mut out = Vec::new();
        for path in paths {
            if locked.contains(&path) {
                out.push(FileChange {
                    path,
                    status: ChangeStatus::Locked,
                    binary: true,
                    added: 0,
                    removed: 0,
                });
                continue;
            }
            let now = self.current_bytes(&path, opts)?;
            let then = saved.get(&path);
            let (status, a, b) = match (&now, then) {
                (Some(n), Some(t)) if n == t => continue,
                (Some(n), Some(t)) => (ChangeStatus::Changed, Some(n), Some(t)),
                (Some(n), None) => (ChangeStatus::OnlyNow, Some(n), None),
                (None, Some(t)) => (ChangeStatus::OnlySnapshot, None, Some(t)),
                (None, None) => continue,
            };
            let text = |v: Option<&Vec<u8>>| match v {
                None => Some(String::new()),
                Some(b) => String::from_utf8(b.clone()).ok(),
            };
            let (binary, added, removed) = match (text(a), text(b)) {
                (Some(old), Some(new)) => {
                    let ops = diff_lines(&old, &new, 0);
                    let count = |f: fn(&DiffOp) -> bool| {
                        ops.as_ref()
                            .map_or(0, |o| o.iter().filter(|x| f(x)).count())
                    };
                    (
                        false,
                        count(|o| matches!(o, DiffOp::Add(_))),
                        count(|o| matches!(o, DiffOp::Del(_))),
                    )
                }
                _ => (true, 0, 0),
            };
            out.push(FileChange {
                path,
                status,
                binary,
                added,
                removed,
            });
        }
        Ok(out)
    }

    /// The lines that differ in one file between the graph as it is now and the snapshot (BK-10).
    pub fn backup_file_diff(
        &self,
        dir: &Path,
        name: &str,
        path: &str,
        opts: &BackupOptions,
    ) -> Result<FileDiff> {
        check_name(name)?;
        let mut z = open_zip(&dir.join(name), opts.keys.as_deref())?;
        let then = saved_file(&mut z, path, opts.graph_keys.as_deref())?;
        let now = self.current_bytes(path, opts)?;
        let text = |v: Option<Vec<u8>>| match v {
            None => Some(String::new()),
            Some(b) => String::from_utf8(b).ok(),
        };
        let (Some(old), Some(new)) = (text(now), text(then)) else {
            return Ok(FileDiff {
                binary: true,
                too_large: false,
                lines: Vec::new(),
            });
        };
        Ok(match diff_lines(&old, &new, 2) {
            Some(lines) => FileDiff {
                binary: false,
                too_large: false,
                lines,
            },
            None => FileDiff {
                binary: false,
                too_large: true,
                lines: Vec::new(),
            },
        })
    }
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
        opts: &BackupOptions,
    ) -> Result<Option<BackupFile>> {
        self.snapshot(dir, now, force, min_age, None, false, opts)
    }

    /// A restore point before a bulk change: like [`Graph::backup_now`] without `force`, so nothing is written when
    /// the newest snapshot (of any kind) already holds the same files. The file is named `...-before.zip`.
    pub fn backup_before_change(
        &self,
        dir: &Path,
        now: NaiveDateTime,
        opts: &BackupOptions,
    ) -> Result<Option<BackupFile>> {
        self.snapshot(dir, now, false, None, None, true, opts)
    }

    /// [`Graph::backup_now`]; the snapshot named `protect` is never removed by the retention.
    #[allow(clippy::too_many_arguments)]
    fn snapshot(
        &self,
        dir: &Path,
        now: NaiveDateTime,
        force: bool,
        min_age: Option<chrono::Duration>,
        protect: Option<&str>,
        before: bool,
        opts: &BackupOptions,
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
        let marks = marks_of(self, opts)?;
        let fp = fingerprint(&files, &opts.extras, marks.as_ref());
        if !force {
            if let Some(newest) = existing.first() {
                if min_age.is_some_and(|a| now.signed_duration_since(newest.created) < a) {
                    return Ok(None);
                }
                // a plain newest snapshot does not stand in for a sealed one (encryption was just turned on), and
                // the other way round
                if newest.encrypted == opts.seal_to.is_some()
                    && stored_fingerprint(&newest.path).as_deref() == Some(fp.as_str())
                {
                    return Ok(None);
                }
            }
        }
        std::fs::create_dir_all(dir)?;
        // two snapshots in one second (a forced one right after another): the later one waits for a free name
        let mut at = now.with_nanosecond(0).unwrap_or(now);
        let sealed = opts.seal_to.is_some();
        while dir.join(file_name(&id, at, before, sealed)).exists() {
            at += chrono::Duration::seconds(1);
        }
        let name = file_name(&id, at, before, sealed);
        let dest = dir.join(&name);
        let partial = dir.join(format!("{name}.partial"));
        let comment = format!("{COMMENT_PREFIX}{fp}");
        let written = (|| -> Result<()> {
            let method = if opts.compress {
                zip::CompressionMethod::Deflated
            } else {
                zip::CompressionMethod::Stored
            };
            let device: Vec<(String, PathBuf)> = opts
                .extras
                .iter()
                .map(|x| (format!("{DEVICE_DIR}/{}", x.name), x.path.clone()))
                .collect();
            match &opts.seal_to {
                None => {
                    let f = std::fs::File::create(&partial)?;
                    let count = write_zip(&f, &files, &device, method, &comment, marks.as_ref())?;
                    f.sync_all()?;
                    drop(f);
                    verify(&partial, count)
                }
                Some(vault) => {
                    // the plain zip exists only in an unnamed temporary file, never in the backup folder
                    let mut tmp = tempfile::tempfile()?;
                    let count = write_zip(&tmp, &files, &device, method, &comment, marks.as_ref())?;
                    tmp.rewind()?;
                    verify_zip(&tmp, count)?;
                    tmp.rewind()?;
                    let out = std::fs::File::create(&partial)?;
                    let mut w = std::io::BufWriter::new(out);
                    notatnik_crypt::seal(
                        vault,
                        comment.as_bytes(),
                        &mut std::io::BufReader::new(&tmp),
                        &mut w,
                    )?;
                    w.flush()?;
                    w.get_ref().sync_all()?;
                    drop(w);
                    check_sealed(&partial, vault, opts.keys.as_deref())
                }
            }
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
            before,
            bytes,
            encrypted: sealed,
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
        opts: &BackupOptions,
    ) -> Result<()> {
        check_name(name)?;
        crate::check_rel(rel)?;
        let mut z = open_zip(&dir.join(name), opts.keys.as_deref())?;
        let text = saved_file(&mut z, rel, opts.graph_keys.as_deref())?
            .ok_or_else(|| Error::NotFound(format!("{rel} in {name}")))?;
        let text = String::from_utf8(text)
            .map_err(|_| Error::InvalidPath(format!("{rel} in {name} is not text")))?;
        self.snapshot(dir, now, true, None, Some(name), true, opts)?;
        let current = match self.read_page(rel) {
            Ok(p) => Some(p.hash),
            Err(Error::NotFound(_)) => None,
            Err(e) => return Err(e),
        };
        self.lock().pin_newest_version(rel);
        self.write_page(rel, &text, current.as_deref())?;
        Ok(())
    }

    /// Unpacks a snapshot of this graph from `dir` into a new or empty folder.
    pub fn restore_backup_to(
        &self,
        dir: &Path,
        name: &str,
        target: &Path,
        opts: &BackupOptions,
    ) -> Result<usize> {
        check_name(name)?;
        let root = self.root().to_path_buf();
        if target.starts_with(&root) {
            return Err(Error::InvalidPath(
                "restore into a folder outside the graph folder".into(),
            ));
        }
        restore_backup(
            &dir.join(name),
            target,
            opts.keys.as_deref(),
            opts.graph_keys.as_deref(),
        )
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
        let name = file_name(id, created, false, false);
        BackupFile {
            path: PathBuf::from(&name),
            name,
            created,
            before: false,
            bytes: 1,
            encrypted: false,
        }
    }

    #[test]
    fn names_round_trip_and_belong_to_one_graph() {
        let n = file_name("gab12cd", at("2026-10-10 09:30:05"), false, false);
        assert_eq!(n, "notatnik-gab12cd-20261010-093005.zip");
        assert_eq!(
            parse_name("gab12cd", &n),
            Some((at("2026-10-10 09:30:05"), false, false))
        );
        let b = file_name("gab12cd", at("2026-10-10 09:30:05"), true, false);
        assert_eq!(b, "notatnik-gab12cd-20261010-093005-before.zip");
        assert_eq!(
            parse_name("gab12cd", &b),
            Some((at("2026-10-10 09:30:05"), true, false))
        );
        assert!(check_name(&b).is_ok());
        let e = file_name("gab12cd", at("2026-10-10 09:30:05"), false, true);
        assert_eq!(e, "notatnik-gab12cd-20261010-093005.zip.enc");
        assert_eq!(
            parse_name("gab12cd", &e),
            Some((at("2026-10-10 09:30:05"), false, true))
        );
        assert!(check_name(&e).is_ok());
        assert!(check_name("notatnik-g-20261010-093005.enc").is_err());
        assert_eq!(parse_name("gother", &n), None);
        assert_eq!(parse_name("gab12cd", "notatnik-gab12cd-x.zip"), None);
        assert!(check_name(&n).is_ok());
        assert!(check_name("../notatnik-g-20261010-093005.zip").is_err());
        assert!(check_name("notes.zip").is_err());
    }

    #[test]
    fn before_snapshots_keep_the_newest_ten_and_stay_out_of_the_schedule() {
        let mut all: Vec<BackupFile> = (0..14)
            .map(|i| {
                let mut b = fake("g1", &format!("2026-10-10 {:02}:00:00", 23 - i));
                b.before = true;
                b
            })
            .collect();
        all.push(fake("g1", "2026-10-10 08:00:00"));
        let gone = expired(&all);
        assert_eq!(gone.len(), 4);
        assert!(gone.iter().all(|b| b.before));
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
