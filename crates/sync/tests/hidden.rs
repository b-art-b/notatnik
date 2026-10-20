//! Hidden file and folder names (EN-25) and re-encrypting (EN-26): what the server lists, two devices, a locked key,
//! switching the setting, a changed key, and objects that were swapped. Against the in-memory WebDAV server.
use std::path::Path;
use std::sync::Arc;

use notatnik_core::encrypted::EncryptedPaths;
use notatnik_crypt::{KdfParams, Keys, Vault};
use notatnik_sync::fakedav::FakeDav;
use notatnik_sync::{Bases, Encryption, Options, Remote, Report, SyncState, WebDavRemote, sync};

const SECRET: &str = "SECRETMARKER-4711";
const PATHS: &[&str] = &["pages/private/", "pages/Health.md"];

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
    fn state_path(&self) -> std::path::PathBuf {
        self.state.path().join("state.json")
    }
}

struct Env {
    dav: FakeDav,
    keys: Arc<Keys>,
}

impl Env {
    fn new() -> Env {
        let (_, keys, _) = Vault::create("pw", KdfParams::insecure_for_tests()).unwrap();
        Env {
            dav: FakeDav::start(),
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
    fn options(&self, d: &Device, hidden: bool, keys: Option<Arc<Keys>>) -> Options {
        let mut o = Options::new(d.state_path());
        o.bases = Bases::new(Some(d.state.path().join("bases")));
        o.encryption = Some(Encryption {
            paths: EncryptedPaths::new(&PATHS.iter().map(|s| (*s).to_string()).collect::<Vec<_>>()),
            keys,
            hidden,
        });
        o
    }
    fn sync_with(&self, d: &Device, hidden: bool, keys: &Arc<Keys>) -> Report {
        let r = sync(
            d.root(),
            &self.remote(),
            &self.options(d, hidden, Some(keys.clone())),
        )
        .unwrap();
        assert!(r.errors.is_empty(), "{r:?}");
        r
    }
    /// A sync with the key unlocked and hidden names.
    fn sync(&self, d: &Device) -> Report {
        self.sync_with(d, true, &self.keys)
    }
    fn names(&self) -> Vec<String> {
        self.dav
            .files("Notes/g")
            .into_iter()
            .map(|p| p.trim_start_matches("Notes/g/").to_string())
            .collect()
    }
    fn objects(&self) -> Vec<String> {
        self.names()
            .into_iter()
            .filter(|n| n.starts_with(".notatnik-enc/"))
            .collect()
    }
}

fn contains(hay: &[u8], needle: &str) -> bool {
    hay.windows(needle.len()).any(|w| w == needle.as_bytes())
}

fn seed(a: &Device) {
    a.write("pages/private/diary.md", &format!("- {SECRET} one\n"));
    a.write(
        "pages/private/deep/er/Zażółć 日本語.md",
        &format!("- {SECRET} two\n"),
    );
    a.write("pages/Health.md", &format!("- {SECRET} three\n"));
    a.write("pages/open.md", "- open\n");
}

#[test]
fn the_server_lists_no_name_or_folder_of_a_marked_file() {
    let e = Env::new();
    let a = Device::new();
    seed(&a);
    let r = e.sync(&a);
    assert!(r.complete && r.pushed.len() == 4, "{r:?}");
    let names = e.names();
    assert_eq!(e.objects().len(), 3, "{names:?}");
    for n in &names {
        for word in ["private", "diary", "Health", "deep", "Zażółć", "日本語"] {
            assert!(!n.contains(word), "{n} shows {word}");
        }
    }
    // the unmarked file is where it was; the marked ones are objects of the fixed shape
    assert!(names.contains(&"pages/open.md".to_string()));
    for o in e.objects() {
        assert!(notatnik_core::encrypted::hidden_name(&o).is_some(), "{o}");
        assert!(!contains(
            &e.dav.file(&format!("Notes/g/{o}")).unwrap(),
            SECRET
        ));
    }
    // no folder of the marked paths exists on the server either
    let pages = e.remote().list("pages").unwrap().unwrap();
    let in_pages: Vec<_> = pages.entries.iter().map(|x| x.name.as_str()).collect();
    assert_eq!(in_pages, ["open.md"], "{in_pages:?}");
    // a quiet sync does nothing
    let r = e.sync(&a);
    assert!(r.complete && !r.changed(), "{r:?}");
}

#[test]
fn a_second_device_learns_the_paths_from_the_objects_and_follows_renames_and_deletions() {
    let e = Env::new();
    let (a, b) = (Device::new(), Device::new());
    seed(&a);
    e.sync(&a);
    let r = e.sync(&b);
    assert_eq!(r.pulled.len(), 4, "{r:?}");
    assert_eq!(
        b.read("pages/private/deep/er/Zażółć 日本語.md"),
        format!("- {SECRET} two\n")
    );
    assert_eq!(b.read("pages/Health.md"), format!("- {SECRET} three\n"));
    assert!(e.sync(&b).pulled.is_empty());

    // an edit on B arrives on A
    b.write("pages/private/diary.md", &format!("- {SECRET} edited\n"));
    e.sync(&b);
    let r = e.sync(&a);
    assert_eq!(r.pulled, ["pages/private/diary.md"], "{r:?}");
    assert_eq!(
        a.read("pages/private/diary.md"),
        format!("- {SECRET} edited\n")
    );

    // a rename on A: the old object goes, a new one comes, B has the file under the new path only
    let before = e.objects();
    std::fs::rename(
        a.root().join("pages/private/diary.md"),
        a.root().join("pages/private/journal.md"),
    )
    .unwrap();
    e.sync(&a);
    let after = e.objects();
    assert_eq!(after.len(), 3);
    assert_ne!(before, after);
    e.sync(&b);
    assert!(!b.has("pages/private/diary.md") && b.has("pages/private/journal.md"));
    assert_eq!(
        b.read("pages/private/journal.md"),
        format!("- {SECRET} edited\n")
    );

    // a deletion on A
    std::fs::remove_file(a.root().join("pages/Health.md")).unwrap();
    e.sync(&a);
    assert_eq!(e.objects().len(), 2);
    e.sync(&b);
    assert!(!b.has("pages/Health.md"));
}

#[test]
fn a_locked_device_leaves_hidden_objects_alone_and_a_quiet_sync_still_sees_them() {
    let e = Env::new();
    let (a, b) = (Device::new(), Device::new());
    seed(&a);
    e.sync(&a);
    let locked = |trust: bool| {
        let mut o = e.options(&b, true, None);
        o.trust_folder_etags = trust;
        sync(b.root(), &e.remote(), &o).unwrap()
    };
    for trust in [false, true, true] {
        let r = locked(trust);
        assert_eq!(r.left_alone, 3, "{r:?}");
        assert!(b.has("pages/open.md") && !b.has("pages/Health.md"));
    }
    // locked: nothing on the server was deleted
    assert_eq!(e.objects().len(), 3);
    // unlocked, the shortcut being on: everything comes
    let mut o = e.options(&b, true, Some(e.keys.clone()));
    o.trust_folder_etags = true;
    let r = sync(b.root(), &e.remote(), &o).unwrap();
    assert_eq!(r.pulled.len(), 3, "{r:?}");
    // and once in step the shortcut is used again
    sync(b.root(), &e.remote(), &o).unwrap();
    e.dav.clear_requests();
    let r = sync(b.root(), &e.remote(), &o).unwrap();
    assert!(r.complete && !r.changed(), "{r:?}");
    let lists = e
        .dav
        .requests()
        .iter()
        .filter(|q| q.starts_with("PROPFIND"))
        .count();
    assert_eq!(lists, 1, "{:?}", e.dav.requests());
}

#[test]
fn switching_between_visible_and_hidden_names_stores_every_file_again_and_leaves_no_old_copy() {
    let e = Env::new();
    let (a, b) = (Device::new(), Device::new());
    seed(&a);
    e.sync_with(&a, false, &e.keys);
    assert!(
        e.names()
            .contains(&"pages/private/diary.md.enc".to_string())
    );

    // to hidden
    let r = e.sync(&a);
    assert_eq!(r.recoded.len(), 3, "{r:?}");
    assert!(
        e.names().iter().all(|n| !n.ends_with(".md.enc")),
        "{:?}",
        e.names()
    );
    assert_eq!(e.objects().len(), 3);
    assert!(e.sync(&a).complete);
    // B arrives at the hidden state directly
    let r = e.sync(&b);
    assert_eq!(r.pulled.len(), 4, "{r:?}");
    assert_eq!(
        b.read("pages/private/diary.md"),
        format!("- {SECRET} one\n")
    );

    // and back
    let r = e.sync_with(&a, false, &e.keys);
    assert_eq!(r.recoded.len(), 3, "{r:?}");
    assert!(e.objects().is_empty(), "{:?}", e.names());
    assert!(
        e.names()
            .contains(&"pages/private/diary.md.enc".to_string())
    );
    assert!(e.sync_with(&a, false, &e.keys).complete);
}

#[test]
fn a_new_key_stores_the_files_again_under_new_names_and_removes_the_old_objects() {
    let e = Env::new();
    let (a, b) = (Device::new(), Device::new());
    seed(&a);
    e.sync(&a);
    e.sync(&b);
    let old = e.objects();

    // re-encrypting: a new key, the stored copies are marked to be stored again (EN-26)
    let (_, new_keys, _) = Vault::create("another", KdfParams::insecure_for_tests()).unwrap();
    let new_keys = Arc::new(new_keys);
    let mut st = SyncState::load(&a.state_path());
    assert_eq!(st.store_encrypted_again(), 3);
    st.save(&a.state_path()).unwrap();
    let r = e.sync_with(&a, true, &new_keys);
    assert_eq!(r.pushed.len(), 3, "{r:?}");
    let now = e.objects();
    assert_eq!(now.len(), 3, "{:?}", e.names());
    assert!(
        now.iter().all(|o| !old.contains(o)),
        "an old object is left: {now:?}"
    );
    assert!(e.sync_with(&a, true, &new_keys).complete);

    // a device that has the new key reads everything (its state is empty, it learns the paths)
    let c = Device::new();
    let r = e.sync_with(&c, true, &new_keys);
    assert_eq!(r.pulled.len(), 4, "{r:?}");
    assert_eq!(c.read("pages/Health.md"), format!("- {SECRET} three\n"));
    // the old key opens nothing any more: the three objects are reported, only the plain file comes
    let d = Device::new();
    let r = sync(
        d.root(),
        &e.remote(),
        &e.options(&d, true, Some(e.keys.clone())),
    )
    .unwrap();
    assert_eq!(r.errors.len(), 3, "{r:?}");
    assert_eq!(r.pulled, ["pages/open.md"], "{r:?}");
    assert_eq!(r.left_alone, 3, "{r:?}");
}

#[test]
fn an_object_moved_to_another_name_is_refused() {
    let e = Env::new();
    let (a, b) = (Device::new(), Device::new());
    seed(&a);
    e.sync(&a);
    let objects = e.objects();
    // the content of one object under the name of another
    let (x, y) = (&objects[0], &objects[1]);
    let bytes = e.dav.file(&format!("Notes/g/{x}")).unwrap();
    e.dav.put_file(&format!("Notes/g/{y}"), &bytes);
    let r = sync(
        b.root(),
        &e.remote(),
        &e.options(&b, true, Some(e.keys.clone())),
    )
    .unwrap();
    assert!(
        r.errors
            .iter()
            .any(|(p, why)| p == y && why.contains("not the object")),
        "{r:?}"
    );
    // the other two arrived, the swapped one did not
    assert_eq!(r.pulled.len(), 3, "{r:?}");
}

#[test]
fn verify_reads_hidden_objects_and_reports_a_copy_in_the_wrong_place() {
    use notatnik_sync::verify;
    let e = Env::new();
    let a = Device::new();
    seed(&a);
    e.sync(&a);
    let paths = EncryptedPaths::new(&PATHS.iter().map(|s| (*s).to_string()).collect::<Vec<_>>());
    let r = verify(a.root(), &e.remote(), &paths, true, Some(&e.keys)).unwrap();
    assert!(
        r.is_clean() && r.same == 4 && r.same_encrypted == 3,
        "{r:?}"
    );
    // without the key the three objects are listed as not checked, by their (meaningless) names
    let r = verify(a.root(), &e.remote(), &paths, true, None).unwrap();
    assert_eq!(r.locked.len(), 3, "{r:?}");
    assert_eq!(r.same, 1);
    // the setting says visible: the files are stored in the other place, and nothing is changed by looking
    let before = e.names();
    let r = verify(a.root(), &e.remote(), &paths, false, Some(&e.keys)).unwrap();
    assert_eq!(r.other_place.len(), 3, "{r:?}");
    assert_eq!(e.names(), before);
}

#[test]
fn a_device_with_the_old_key_loses_nothing_when_the_graph_was_re_encrypted_elsewhere() {
    let e = Env::new();
    let (a, b) = (Device::new(), Device::new());
    seed(&a);
    e.sync(&a);
    e.sync(&b);

    // A re-encrypts: new names for every marked file
    let (_, new_keys, _) = Vault::create("another", KdfParams::insecure_for_tests()).unwrap();
    let new_keys = Arc::new(new_keys);
    let mut st = SyncState::load(&a.state_path());
    st.store_encrypted_again();
    st.save(&a.state_path()).unwrap();
    e.sync_with(&a, true, &new_keys);

    // B still has the old key: its files look gone from the server, but they are not deleted here, and the new
    // objects are reported
    let r = sync(
        b.root(),
        &e.remote(),
        &e.options(&b, true, Some(e.keys.clone())),
    )
    .unwrap();
    assert!(r.deleted_local.is_empty(), "{r:?}");
    assert_eq!(r.errors.len(), 3, "{r:?}");
    assert!(b.has("pages/Health.md") && b.has("pages/private/diary.md"));
    assert_eq!(
        b.read("pages/private/diary.md"),
        format!("- {SECRET} one\n")
    );
    assert!(!r.complete);
    // with the new key it carries on as if nothing happened: nothing is lost, no conflict
    let r = e.sync_with(&b, true, &new_keys);
    assert!(
        r.deleted_local.is_empty() && r.conflicts.is_empty(),
        "{r:?}"
    );
    assert!(r.complete, "{r:?}");
    assert_eq!(b.read("pages/Health.md"), format!("- {SECRET} three\n"));
}
