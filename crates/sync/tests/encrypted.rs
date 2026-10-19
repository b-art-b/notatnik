//! Encrypting chosen paths on the server (EN-11 to EN-16): what the server holds, two devices, the locked key,
//! changing a mark, and files that were tampered with. Against the in-memory WebDAV server.
use std::path::Path;
use std::sync::Arc;

use notatnik_core::encrypted::EncryptedPaths;
use notatnik_crypt::{KdfParams, Keys, Vault};
use notatnik_sync::fakedav::FakeDav;
use notatnik_sync::{Bases, Encryption, Error, Options, Remote, Report, WebDavRemote, sync};

const SECRET: &str = "SECRETMARKER-4711";
const OPEN: &str = "OPENMARKER-0815";

struct Device {
    replica: tempfile::TempDir,
    state: tempfile::TempDir,
}

impl Device {
    fn new() -> Device {
        Device {
            replica: tempfile::tempdir().unwrap(),
            state: tempfile::tempdir().unwrap(),
        }
    }
    fn root(&self) -> &Path {
        self.replica.path()
    }
    fn write(&self, rel: &str, text: &str) {
        let p = self.root().join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }
    fn read(&self, rel: &str) -> String {
        std::fs::read_to_string(self.root().join(rel)).unwrap()
    }
    fn has(&self, rel: &str) -> bool {
        self.root().join(rel).exists()
    }
}

struct Env {
    dav: FakeDav,
    vault: Vault,
    keys: Arc<Keys>,
}

impl Env {
    fn new() -> Env {
        let (vault, keys, _) = Vault::create("pw", KdfParams::insecure_for_tests()).unwrap();
        Env {
            dav: FakeDav::start(),
            vault,
            keys: Arc::new(keys),
        }
    }
    fn remote(&self) -> WebDavRemote {
        WebDavRemote::new(
            &self.dav.folder_url("Notes/g"),
            &self.dav.user,
            &self.dav.password,
        )
        .unwrap()
    }
    fn options(&self, d: &Device, paths: &[&str], keys: Option<Arc<Keys>>) -> Options {
        let mut o = Options::new(d.state.path().join("state.json"));
        o.bases = Bases::new(Some(d.state.path().join("bases")));
        o.encryption = Some(Encryption {
            paths: EncryptedPaths::new(&paths.iter().map(|s| (*s).to_string()).collect::<Vec<_>>()),
            keys,
        });
        o.now = Box::new(|| {
            chrono::NaiveDate::from_ymd_opt(2026, 10, 17)
                .unwrap()
                .and_hms_opt(12, 30, 0)
                .unwrap()
        });
        o
    }
    /// A sync with the key unlocked.
    fn sync(&self, d: &Device, paths: &[&str]) -> Report {
        let r = sync(
            d.root(),
            &self.remote(),
            &self.options(d, paths, Some(self.keys.clone())),
        )
        .unwrap();
        assert!(r.errors.is_empty(), "{r:?}");
        r
    }
    /// A sync with the key locked.
    fn sync_locked(&self, d: &Device, paths: &[&str]) -> Report {
        sync(d.root(), &self.remote(), &self.options(d, paths, None)).unwrap()
    }
    fn all_server_files(&self) -> Vec<(String, Vec<u8>)> {
        self.dav
            .files("Notes/g")
            .into_iter()
            .map(|p| {
                let data = self.dav.file(&p).unwrap();
                (p.trim_start_matches("Notes/g/").to_string(), data)
            })
            .collect()
    }
    fn server_has(&self, rel: &str) -> bool {
        self.dav.file(&format!("Notes/g/{rel}")).is_some()
    }
    fn server_names(&self) -> Vec<String> {
        self.all_server_files().into_iter().map(|f| f.0).collect()
    }
}

fn contains(hay: &[u8], needle: &str) -> bool {
    hay.windows(needle.len()).any(|w| w == needle.as_bytes())
}

const PATHS: &[&str] = &["pages/private/", "pages/Health.md"];

#[test]
fn marked_files_are_stored_encrypted_and_the_rest_as_they_are() {
    let e = Env::new();
    let a = Device::new();
    a.write("pages/private/diary.md", &format!("- {SECRET} one\n"));
    a.write("pages/private/deep/er/x.md", &format!("- {SECRET} two\n"));
    a.write("pages/Health.md", &format!("- {SECRET} three\n"));
    a.write("pages/open.md", &format!("- {OPEN}\n"));
    a.write(".notatnik/config.toml", &format!("[graph]\n# {OPEN}\n"));
    let r = e.sync(&a, PATHS);
    assert!(r.complete && r.pushed.len() == 5, "{r:?}");

    let names = e.server_names();
    for n in [
        "pages/private/diary.md.enc",
        "pages/private/deep/er/x.md.enc",
        "pages/Health.md.enc",
        "pages/open.md",
        ".notatnik/config.toml",
    ] {
        assert!(names.contains(&n.to_string()), "{n} in {names:?}");
    }
    assert!(
        !names
            .iter()
            .any(|n| n == "pages/private/diary.md" || n == "pages/Health.md"),
        "{names:?}"
    );
    // nothing of the marked text is on the server; the unmarked text is
    let files = e.all_server_files();
    assert!(!files.iter().any(|(_, d)| contains(d, SECRET)));
    assert!(files.iter().any(|(_, d)| contains(d, OPEN)));
    assert_eq!(a.read("pages/Health.md"), format!("- {SECRET} three\n"));

    // nothing changed: nothing is sent, although re-encrypting would give other bytes
    e.dav.clear_requests();
    let r = e.sync(&a, PATHS);
    assert!(r.complete && !r.changed(), "{r:?}");
    assert!(!e.dav.requests().iter().any(|q| q.starts_with("PUT")));
}

#[test]
fn a_second_device_with_the_key_reads_and_edits_both_kinds() {
    let e = Env::new();
    let (a, b) = (Device::new(), Device::new());
    a.write("pages/private/diary.md", &format!("- {SECRET}\n- one\n"));
    a.write("pages/open.md", &format!("- {OPEN}\n- one\n"));
    e.sync(&a, PATHS);

    let r = e.sync(&b, PATHS);
    assert_eq!(r.pulled.len(), 2, "{r:?}");
    assert_eq!(
        b.read("pages/private/diary.md"),
        format!("- {SECRET}\n- one\n")
    );

    // an edit of the encrypted page on B reaches A
    b.write(
        "pages/private/diary.md",
        &format!("- {SECRET}\n- one\n- from b\n"),
    );
    e.sync(&b, PATHS);
    e.sync(&a, PATHS);
    assert!(a.read("pages/private/diary.md").contains("from b"));

    // a change on both sides to different lines merges, as it does for plain files
    a.write(
        "pages/private/diary.md",
        &format!("- {SECRET}\n- a first\n- one\n- from b\n"),
    );
    b.write(
        "pages/private/diary.md",
        &format!("- {SECRET}\n- one\n- from b\n- b last\n"),
    );
    e.sync(&a, PATHS);
    let r = e.sync(&b, PATHS);
    assert_eq!(r.merged, ["pages/private/diary.md"], "{r:?}");
    e.sync(&a, PATHS);
    assert_eq!(
        a.read("pages/private/diary.md"),
        b.read("pages/private/diary.md")
    );
    assert!(a.read("pages/private/diary.md").contains("a first"));
    assert!(a.read("pages/private/diary.md").contains("b last"));
    assert!(
        !e.all_server_files()
            .iter()
            .any(|(_, d)| contains(d, SECRET))
    );
}

#[test]
fn a_conflict_copy_of_an_encrypted_file_is_encrypted_too() {
    let e = Env::new();
    let (a, b) = (Device::new(), Device::new());
    a.write("pages/Health.md", &format!("- {SECRET}\n- same line\n"));
    e.sync(&a, PATHS);
    e.sync(&b, PATHS);
    // the same line changed on both sides: no clean merge
    a.write("pages/Health.md", &format!("- {SECRET}\n- a says\n"));
    b.write("pages/Health.md", &format!("- {SECRET}\n- b says\n"));
    e.sync(&a, PATHS);
    let r = e.sync(&b, PATHS);
    assert_eq!(r.conflicts.len(), 1, "{r:?}");
    assert_eq!(r.conflict_files.len(), 1);
    // the copy goes up on the next sync, and not as it is
    e.sync(&b, PATHS);
    let names = e.server_names();
    let copy = r.conflict_files[0].clone();
    assert!(
        names.contains(&format!("{copy}.enc")),
        "{copy} in {names:?}"
    );
    assert!(!names.contains(&copy));
    assert!(
        !e.all_server_files()
            .iter()
            .any(|(_, d)| contains(d, SECRET))
    );
}

#[test]
fn while_the_key_is_locked_plain_files_sync_and_encrypted_ones_are_left_alone() {
    let e = Env::new();
    let (a, b) = (Device::new(), Device::new());
    a.write("pages/private/diary.md", &format!("- {SECRET}\n"));
    a.write("pages/open.md", "- open\n");
    e.sync(&a, PATHS);

    // a new device that has not unlocked: it gets the plain file only
    let r = e.sync_locked(&b, PATHS);
    assert_eq!(r.pulled, ["pages/open.md"], "{r:?}");
    assert_eq!(r.left_alone, 1);
    assert!(!b.has("pages/private/diary.md"));
    assert!(r.errors.is_empty());

    // nothing marked is uploaded as it is, even a new file
    b.write("pages/private/new.md", &format!("- {SECRET} new\n"));
    b.write("pages/Health.md", &format!("- {SECRET} health\n"));
    e.dav.clear_requests();
    let r = e.sync_locked(&b, PATHS);
    assert!(r.pushed.is_empty() && r.left_alone >= 2, "{r:?}");
    assert!(!e.server_has("pages/private/new.md") && !e.server_has("pages/Health.md"));
    assert!(!e.server_has("pages/private/new.md.enc"));
    assert!(
        !e.dav.requests().iter().any(|q| q.starts_with("PUT")),
        "{:?}",
        e.dav.requests()
    );

    // plain edits go on meanwhile
    b.write("pages/open.md", "- open, edited on b\n");
    e.sync_locked(&b, PATHS);
    e.sync(&a, PATHS);
    assert_eq!(a.read("pages/open.md"), "- open, edited on b\n");

    // unlocking catches up in both directions
    let r = e.sync(&b, PATHS);
    assert!(
        r.pulled.contains(&"pages/private/diary.md".to_string()),
        "{r:?}"
    );
    assert!(e.server_has("pages/private/new.md.enc") && e.server_has("pages/Health.md.enc"));
}

#[test]
fn a_locked_sync_never_deletes_encrypted_files_whatever_the_server_holds() {
    let e = Env::new();
    let (a, b) = (Device::new(), Device::new());
    for i in 0..12 {
        a.write(
            &format!("pages/private/p{i}.md"),
            &format!("- {SECRET} {i}\n"),
        );
    }
    a.write("pages/open.md", "- open\n");
    e.sync(&a, PATHS);
    e.sync(&b, PATHS);
    assert!(b.has("pages/private/p11.md"));

    // the server loses every encrypted file (a wiped folder, a bad restore)
    for i in 0..12 {
        e.dav
            .delete_path(&format!("Notes/g/pages/private/p{i}.md.enc"));
    }
    let r = e.sync_locked(&b, PATHS);
    assert!(r.deleted_local.is_empty() && r.errors.is_empty(), "{r:?}");
    for i in 0..12 {
        assert!(b.has(&format!("pages/private/p{i}.md")));
    }
    // unlocked, the guard against mass deletion still stops it
    let r = sync(
        b.root(),
        &e.remote(),
        &e.options(&b, PATHS, Some(e.keys.clone())),
    );
    assert!(matches!(r, Err(Error::MassDelete(12))), "{r:?}");
    assert!(b.has("pages/private/p0.md"));
}

#[test]
fn encrypted_copies_on_a_server_are_left_alone_when_no_paths_are_marked_and_no_key_is_known() {
    let e = Env::new();
    let (a, b) = (Device::new(), Device::new());
    a.write("pages/private/diary.md", &format!("- {SECRET}\n"));
    a.write("pages/open.md", "- open\n");
    e.sync(&a, PATHS);
    let mut o = Options::new(b.state.path().join("state.json"));
    o.encryption = None;
    let r = sync(b.root(), &e.remote(), &o).unwrap();
    assert_eq!(r.pulled, ["pages/open.md"], "{r:?}");
    assert_eq!(r.left_alone, 1);
    assert!(!b.has("pages/private/diary.md.enc"));
}

#[test]
fn adding_and_removing_a_mark_changes_how_the_server_holds_the_file() {
    let e = Env::new();
    let a = Device::new();
    a.write("pages/Health.md", &format!("- {SECRET}\n"));
    a.write("pages/open.md", "- open\n");
    e.sync(&a, &[]);
    assert!(e.server_has("pages/Health.md"));

    // marked: stored encrypted, the plain copy goes
    let r = e.sync(&a, PATHS);
    assert_eq!(r.recoded, ["pages/Health.md"], "{r:?}");
    assert!(e.server_has("pages/Health.md.enc") && !e.server_has("pages/Health.md"));
    assert!(
        !e.all_server_files()
            .iter()
            .any(|(_, d)| contains(d, SECRET))
    );
    assert_eq!(a.read("pages/Health.md"), format!("- {SECRET}\n"));
    // and stays so
    e.dav.clear_requests();
    let r = e.sync(&a, PATHS);
    assert!(!r.changed(), "{r:?}");

    // unmarked: stored as it is again, the encrypted copy goes
    let r = e.sync(&a, &[]);
    assert_eq!(r.recoded, ["pages/Health.md"], "{r:?}");
    assert!(e.server_has("pages/Health.md") && !e.server_has("pages/Health.md.enc"));
    assert_eq!(
        e.dav.file("Notes/g/pages/Health.md").unwrap(),
        format!("- {SECRET}\n").into_bytes()
    );
}

#[test]
fn a_change_made_while_the_mark_is_added_is_sent_in_the_new_form_only() {
    let e = Env::new();
    let a = Device::new();
    a.write("pages/Health.md", "- before\n");
    e.sync(&a, &[]);
    a.write("pages/Health.md", &format!("- {SECRET} after\n"));
    let r = e.sync(&a, PATHS);
    assert_eq!(r.pushed, ["pages/Health.md"], "{r:?}");
    assert!(e.server_has("pages/Health.md.enc") && !e.server_has("pages/Health.md"));
    assert!(
        !e.all_server_files()
            .iter()
            .any(|(_, d)| contains(d, SECRET))
    );
}

#[test]
fn an_interrupted_change_of_form_is_finished_by_the_next_sync() {
    let e = Env::new();
    let a = Device::new();
    a.write("pages/Health.md", &format!("- {SECRET}\n"));
    e.sync(&a, PATHS);
    // the state in which a run stopped after writing the new form and before deleting the old one
    e.dav.put_file(
        "Notes/g/pages/Health.md",
        format!("- {SECRET}\n").as_bytes(),
    );
    e.sync(&a, PATHS);
    assert!(!e.server_has("pages/Health.md") && e.server_has("pages/Health.md.enc"));

    // the same for the other direction
    a.write("pages/open.md", "- open\n");
    e.sync(&a, PATHS);
    let sealed = e.keys.encrypt(b"- open\n", b"pages/open.md").unwrap();
    e.dav.put_file("Notes/g/pages/open.md.enc", &sealed);
    e.sync(&a, PATHS);
    assert!(e.server_has("pages/open.md") && !e.server_has("pages/open.md.enc"));
}

#[test]
fn two_forms_with_different_contents_are_never_deleted() {
    let e = Env::new();
    let a = Device::new();
    a.write("pages/open.md", "- one\n");
    e.sync(&a, PATHS);
    let sealed = e
        .keys
        .encrypt(b"- another device wrote this\n", b"pages/open.md")
        .unwrap();
    e.dav.put_file("Notes/g/pages/open.md.enc", &sealed);
    let r = sync(
        a.root(),
        &e.remote(),
        &e.options(&a, PATHS, Some(e.keys.clone())),
    )
    .unwrap();
    assert!(
        r.errors
            .iter()
            .any(|(p, m)| p == "pages/open.md" && m.contains("both")),
        "{r:?}"
    );
    assert!(e.server_has("pages/open.md") && e.server_has("pages/open.md.enc"));
    assert_eq!(a.read("pages/open.md"), "- one\n");
}

#[test]
fn files_that_cannot_be_decrypted_are_reported_and_nothing_is_overwritten() {
    let e = Env::new();
    let (a, b) = (Device::new(), Device::new());
    a.write("pages/private/one.md", &format!("- {SECRET} one\n"));
    a.write("pages/private/two.md", &format!("- {SECRET} two\n"));
    a.write("pages/private/three.md", &format!("- {SECRET} three\n"));
    a.write("pages/private/four.md", &format!("- {SECRET} four\n"));
    a.write("pages/open.md", "- open\n");
    e.sync(&a, PATHS);
    // two files swapped on the server, one cut short, one made with another key
    let (one, two) = (
        e.dav.file("Notes/g/pages/private/one.md.enc").unwrap(),
        e.dav.file("Notes/g/pages/private/two.md.enc").unwrap(),
    );
    e.dav.put_file("Notes/g/pages/private/one.md.enc", &two);
    e.dav.put_file("Notes/g/pages/private/two.md.enc", &one);
    let three = e.dav.file("Notes/g/pages/private/three.md.enc").unwrap();
    e.dav.put_file(
        "Notes/g/pages/private/three.md.enc",
        &three[..three.len() - 5],
    );
    let (_, other, _) = Vault::create("x", KdfParams::insecure_for_tests()).unwrap();
    let foreign = other.encrypt(b"- x\n", b"pages/private/four.md").unwrap();
    e.dav
        .put_file("Notes/g/pages/private/four.md.enc", &foreign);

    let r = sync(
        b.root(),
        &e.remote(),
        &e.options(&b, PATHS, Some(e.keys.clone())),
    )
    .unwrap();
    assert_eq!(r.errors.len(), 4, "{r:?}");
    assert!(
        r.errors.iter().all(|(_, m)| m.contains("encryption")),
        "{r:?}"
    );
    for f in ["one", "two", "three", "four"] {
        assert!(!b.has(&format!("pages/private/{f}.md")));
    }
    // the plain file still came
    assert_eq!(b.read("pages/open.md"), "- open\n");
    assert!(!r.complete);
}

#[test]
fn renaming_a_marked_file_replaces_its_encrypted_copy() {
    let e = Env::new();
    let a = Device::new();
    a.write("pages/private/old.md", &format!("- {SECRET}\n"));
    e.sync(&a, PATHS);
    std::fs::rename(
        a.root().join("pages/private/old.md"),
        a.root().join("pages/private/new.md"),
    )
    .unwrap();
    let r = e.sync(&a, PATHS);
    assert_eq!(r.pushed, ["pages/private/new.md"], "{r:?}");
    assert_eq!(r.deleted_remote, ["pages/private/old.md"], "{r:?}");
    let names = e.server_names();
    assert!(names.contains(&"pages/private/new.md.enc".to_string()));
    assert!(!names.contains(&"pages/private/old.md.enc".to_string()));

    // moved out of the marked folder: stored as it is, the encrypted copy goes
    std::fs::rename(
        a.root().join("pages/private/new.md"),
        a.root().join("pages/moved.md"),
    )
    .unwrap();
    e.sync(&a, PATHS);
    assert!(e.server_has("pages/moved.md") && !e.server_has("pages/private/new.md.enc"));
}

#[test]
fn a_local_file_that_ends_in_enc_is_not_synced_and_is_reported() {
    let e = Env::new();
    let a = Device::new();
    a.write("assets/backup.enc", "not ours");
    a.write("pages/open.md", "- open\n");
    let r = e.sync(&a, PATHS);
    assert_eq!(r.reserved, ["assets/backup.enc"], "{r:?}");
    assert!(!e.server_has("assets/backup.enc"));
    assert!(e.server_has("pages/open.md"));
}

#[test]
fn a_marked_file_deleted_on_one_side_is_deleted_on_the_other() {
    let e = Env::new();
    let (a, b) = (Device::new(), Device::new());
    a.write("pages/private/gone.md", &format!("- {SECRET}\n"));
    e.sync(&a, PATHS);
    e.sync(&b, PATHS);
    std::fs::remove_file(a.root().join("pages/private/gone.md")).unwrap();
    e.sync(&a, PATHS);
    assert!(!e.server_has("pages/private/gone.md.enc"));
    let r = e.sync(&b, PATHS);
    assert_eq!(r.deleted_local, ["pages/private/gone.md"], "{r:?}");
    let _ = (&e.vault, e.remote().list("").unwrap());
}

#[test]
fn a_file_left_alone_is_not_hidden_by_the_folder_etag_shortcut() {
    let e = Env::new();
    let (a, b) = (Device::new(), Device::new());
    a.write("pages/private/diary.md", &format!("- {SECRET}\n"));
    a.write("pages/open.md", "- open\n");
    e.sync(&a, PATHS);
    let trusting = |d: &Device, keys: Option<Arc<Keys>>| {
        let mut o = e.options(d, PATHS, keys);
        o.trust_folder_etags = true;
        o
    };
    // B syncs locked, twice: nothing changed on the server, yet the encrypted file is still counted
    let r = sync(b.root(), &e.remote(), &trusting(&b, None)).unwrap();
    assert_eq!(r.left_alone, 1, "{r:?}");
    let r = sync(b.root(), &e.remote(), &trusting(&b, None)).unwrap();
    assert_eq!(r.left_alone, 1, "{r:?}");
    // and when the key is there, it comes, although nothing changed on the server since
    let r = sync(b.root(), &e.remote(), &trusting(&b, Some(e.keys.clone()))).unwrap();
    assert_eq!(r.pulled, ["pages/private/diary.md"], "{r:?}");
    assert_eq!(b.read("pages/private/diary.md"), format!("- {SECRET}\n"));
    // fully in step, the shortcut is used again: a quiet sync asks the server for the top folder only
    sync(b.root(), &e.remote(), &trusting(&b, Some(e.keys.clone()))).unwrap();
    e.dav.clear_requests();
    let r = sync(b.root(), &e.remote(), &trusting(&b, Some(e.keys.clone()))).unwrap();
    assert!(r.complete && !r.changed(), "{r:?}");
    let lists = e
        .dav
        .requests()
        .iter()
        .filter(|q| q.starts_with("PROPFIND"))
        .count();
    assert_eq!(lists, 1, "{:?}", e.dav.requests());
}
