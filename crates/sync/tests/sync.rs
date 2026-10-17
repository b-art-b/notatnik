//! The sync engine against the in-memory WebDAV server: first sync both ways, edits, deletes, conflicts, the
//! guard against mass deletion, and skipping folders whose ETag did not change (SY-1..SY-7).
use std::path::{Path, PathBuf};

use notatnik_sync::fakedav::FakeDav;
use notatnik_sync::{
    Bases, Error, Options, Remote, WebDavRemote, check_url, folder_url, probe, sync,
};

struct Env {
    dav: FakeDav,
    replica: tempfile::TempDir,
    state: tempfile::TempDir,
}

impl Env {
    fn new() -> Env {
        Env {
            dav: FakeDav::start(),
            replica: tempfile::tempdir().unwrap(),
            state: tempfile::tempdir().unwrap(),
        }
    }
    fn root(&self) -> &Path {
        self.replica.path()
    }
    fn remote(&self) -> WebDavRemote {
        WebDavRemote::new(
            &self.dav.folder_url("Notes/g"),
            &self.dav.user,
            &self.dav.password,
        )
        .unwrap()
    }
    fn options(&self, trust: bool) -> Options {
        let mut o = Options::new(self.state.path().join("state.json"));
        o.trust_folder_etags = trust;
        o.now = Box::new(|| {
            chrono::NaiveDate::from_ymd_opt(2026, 10, 17)
                .unwrap()
                .and_hms_opt(12, 30, 0)
                .unwrap()
        });
        o
    }
    /// Options that keep the pages as last synced, so changes on both sides can be merged.
    fn merging(&self) -> Options {
        let mut o = self.options(false);
        o.bases = Bases::new(Some(self.state.path().join("bases")));
        o
    }
    fn sync(&self) -> notatnik_sync::Report {
        sync(self.root(), &self.remote(), &self.options(false)).unwrap()
    }
    fn write(&self, rel: &str, text: &str) {
        let p = self.root().join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }
    fn read(&self, rel: &str) -> String {
        std::fs::read_to_string(self.root().join(rel)).unwrap()
    }
    fn remote_text(&self, rel: &str) -> Option<String> {
        self.dav
            .file(&format!("Notes/g/{rel}"))
            .map(|b| String::from_utf8(b).unwrap())
    }
}

#[test]
fn the_first_sync_uploads_a_graph_and_leaves_what_is_not_synced() {
    let e = Env::new();
    e.write("pages/inbox.md", "- one\n");
    e.write("journals/2026/2026-10-17.md", "- today\n");
    e.write(".notatnik/config.toml", "[graph]\n");
    e.write(".notatnik/tmp/w1-0", "half written");
    e.write("pages/inbox.md~", "editor backup");
    std::fs::create_dir_all(e.root().join("assets/2026/10")).unwrap();
    let png: Vec<u8> = (0..=255u8).cycle().take(70_000).collect();
    std::fs::write(e.root().join("assets/2026/10/pasted-1a2b3c4d.png"), &png).unwrap();

    let r = e.sync();
    assert!(r.complete && r.errors.is_empty(), "{r:?}");
    assert_eq!(r.pushed.len(), 4, "{r:?}");
    assert_eq!(e.remote_text("pages/inbox.md").as_deref(), Some("- one\n"));
    assert_eq!(
        e.remote_text(".notatnik/config.toml").as_deref(),
        Some("[graph]\n")
    );
    assert_eq!(
        e.dav
            .file("Notes/g/assets/2026/10/pasted-1a2b3c4d.png")
            .unwrap(),
        png
    );
    assert!(e.remote_text(".notatnik/tmp/w1-0").is_none());
    assert!(e.remote_text("pages/inbox.md~").is_none());

    // nothing changed: nothing is sent
    e.dav.clear_requests();
    let r = e.sync();
    assert!(r.complete && !r.changed(), "{r:?}");
    assert!(
        !e.dav.requests().iter().any(|q| q.starts_with("PUT")),
        "{:?}",
        e.dav.requests()
    );
}

#[test]
fn an_empty_replica_downloads_the_graph_with_unusual_names() {
    let e = Env::new();
    e.dav.put_file(
        "Notes/g/pages/zażółć gęślą (1) #x.md",
        "- ✓ text\n".as_bytes(),
    );
    e.dav
        .put_file("Notes/g/pages/work/projects/alpha.md", b"- alpha\n");
    e.dav
        .put_file("Notes/g/.notatnik/config.toml", b"[graph]\n");
    let r = e.sync();
    assert!(r.complete, "{r:?}");
    assert_eq!(r.pulled.len(), 3);
    assert_eq!(e.read("pages/zażółć gęślą (1) #x.md"), "- ✓ text\n");
    assert_eq!(e.read("pages/work/projects/alpha.md"), "- alpha\n");
}

#[test]
fn edits_on_either_side_travel_the_other_way() {
    let e = Env::new();
    e.write("pages/a.md", "- a\n");
    e.write("pages/b.md", "- b\n");
    e.sync();
    e.write("pages/a.md", "- a, edited here\n");
    e.dav.put_file("Notes/g/pages/b.md", b"- b, edited there\n");
    e.dav.put_file("Notes/g/pages/new.md", b"- new there\n");
    e.write("pages/mine.md", "- new here\n");
    let r = e.sync();
    assert!(r.complete && r.conflicts.is_empty(), "{r:?}");
    assert_eq!(e.remote_text("pages/a.md").unwrap(), "- a, edited here\n");
    assert_eq!(e.read("pages/b.md"), "- b, edited there\n");
    assert_eq!(e.read("pages/new.md"), "- new there\n");
    assert_eq!(e.remote_text("pages/mine.md").unwrap(), "- new here\n");
    assert!(!e.sync().changed());
}

#[test]
fn the_same_edit_on_both_sides_is_not_a_conflict() {
    let e = Env::new();
    e.write("pages/a.md", "- a\n");
    e.sync();
    e.write("pages/a.md", "- same\n");
    e.dav.put_file("Notes/g/pages/a.md", b"- same\n");
    let r = e.sync();
    assert!(r.complete && r.conflicts.is_empty(), "{r:?}");
    assert!(!e.sync().changed());
}

#[test]
fn different_edits_keep_the_servers_version_and_save_mine_beside_it() {
    let e = Env::new();
    e.write("pages/a.md", "- a\n");
    e.sync();
    e.write("pages/a.md", "- a, my edit\n");
    e.dav.put_file("Notes/g/pages/a.md", b"- a, their edit\n");
    let r = e.sync();
    assert!(r.complete, "{r:?}");
    assert_eq!(r.conflicts, ["pages/a.md"]);
    assert_eq!(
        r.conflict_files,
        ["pages/a (conflicted copy 2026-10-17 123000).md"]
    );
    assert_eq!(e.read("pages/a.md"), "- a, their edit\n");
    assert_eq!(
        e.read("pages/a (conflicted copy 2026-10-17 123000).md"),
        "- a, my edit\n"
    );
    // the copy is uploaded too, so every device sees the conflict
    assert_eq!(
        e.remote_text("pages/a (conflicted copy 2026-10-17 123000).md")
            .unwrap(),
        "- a, my edit\n"
    );
    assert_eq!(e.remote_text("pages/a.md").unwrap(), "- a, their edit\n");
    assert!(!e.sync().changed());
}

/// ST-6, P5.2: edits to different blocks of one page, on two devices, are merged and nothing is left over.
#[test]
fn edits_to_different_blocks_on_two_devices_are_merged() {
    let e = Env::new();
    e.write("pages/a.md", "- one\n- two\n- three\n");
    let go = |e: &Env| sync(e.root(), &e.remote(), &e.merging()).unwrap();
    assert!(go(&e).complete);
    e.write("pages/a.md", "- one mine\n- two\n- three\n");
    e.dav
        .put_file("Notes/g/pages/a.md", b"- one\n- two\n- three theirs\n");
    let r = go(&e);
    assert!(r.complete && r.conflicts.is_empty(), "{r:?}");
    assert_eq!(r.merged, ["pages/a.md"]);
    let want = "- one mine\n- two\n- three theirs\n";
    assert_eq!(e.read("pages/a.md"), want);
    assert_eq!(e.remote_text("pages/a.md").unwrap(), want);
    assert_eq!(e.dav.files("Notes/g/pages"), ["Notes/g/pages/a.md"]);
    // and the next sync has nothing to do
    assert!(!go(&e).changed());
}

/// ST-6: the same block edited on both sides cannot be merged: the old behaviour, plus the common ancestor for
/// the conflict view.
#[test]
fn the_same_block_edited_on_two_devices_keeps_a_copy_and_the_ancestor() {
    let e = Env::new();
    e.write("pages/a.md", "- one\n- two\n");
    let go = |e: &Env| sync(e.root(), &e.remote(), &e.merging()).unwrap();
    go(&e);
    e.write("pages/a.md", "- one mine\n- two\n");
    e.dav
        .put_file("Notes/g/pages/a.md", b"- one theirs\n- two\n");
    let r = go(&e);
    assert!(r.merged.is_empty());
    let copy = "pages/a (conflicted copy 2026-10-17 123000).md";
    assert_eq!(r.conflict_files, [copy]);
    assert_eq!(e.read("pages/a.md"), "- one theirs\n- two\n");
    assert_eq!(e.read(copy), "- one mine\n- two\n");
    let bases = e.merging().bases;
    assert_eq!(bases.ancestor(copy).unwrap(), b"- one\n- two\n");
}

/// A page that was synced before bases were kept gets one at the next sync, so it can be merged later.
#[test]
fn a_page_without_a_kept_base_is_given_one_while_it_is_unchanged() {
    let e = Env::new();
    e.write("pages/a.md", "- one\n- two\n");
    e.sync(); // no bases kept
    let go = |e: &Env| sync(e.root(), &e.remote(), &e.merging()).unwrap();
    go(&e);
    assert!(e.merging().bases.has_synced("pages/a.md"));
    e.write("pages/a.md", "- one mine\n- two\n");
    e.dav
        .put_file("Notes/g/pages/a.md", b"- one\n- two theirs\n");
    assert_eq!(go(&e).merged, ["pages/a.md"]);
    assert_eq!(e.read("pages/a.md"), "- one mine\n- two theirs\n");
}

/// A folder chosen in the folder browser can be several new levels deep: the missing ones are made.
#[test]
fn a_synced_folder_below_missing_folders_is_made_with_them() {
    let e = Env::new();
    let remote = WebDavRemote::new(
        &e.dav.folder_url("Notes/Work/Clients/graph"),
        &e.dav.user,
        &e.dav.password,
    )
    .unwrap();
    remote.mkdir("").unwrap();
    assert!(remote.list("").unwrap().is_some());
    // and a folder that is there already is fine
    remote.mkdir("").unwrap();
    remote.mkdir("pages/2026").unwrap();
    assert!(remote.list("pages").unwrap().is_some());
}

#[test]
fn deletes_travel_but_an_edit_beats_a_delete() {
    let e = Env::new();
    for n in ["a", "b", "c", "d"] {
        e.write(&format!("pages/{n}.md"), &format!("- {n}\n"));
    }
    e.sync();
    // deleted here / there
    std::fs::remove_file(e.root().join("pages/a.md")).unwrap();
    e.dav.delete_path("Notes/g/pages/b.md");
    // deleted on one side, edited on the other
    std::fs::remove_file(e.root().join("pages/c.md")).unwrap();
    e.dav.put_file("Notes/g/pages/c.md", b"- c, edited there\n");
    e.write("pages/d.md", "- d, edited here\n");
    e.dav.delete_path("Notes/g/pages/d.md");
    let r = e.sync();
    assert!(r.complete, "{r:?}");
    assert!(e.remote_text("pages/a.md").is_none());
    assert!(!e.root().join("pages/b.md").exists());
    assert_eq!(e.read("pages/c.md"), "- c, edited there\n");
    assert_eq!(e.remote_text("pages/d.md").unwrap(), "- d, edited here\n");
    // what was deleted here went to the trash, not into the void
    assert_eq!(e.read(".notatnik/trash/pages/b.md"), "- b\n");
    assert!(!e.sync().changed());
}

#[test]
fn a_sync_that_would_delete_most_of_the_graph_stops() {
    let e = Env::new();
    for n in 0..14 {
        e.write(&format!("pages/p{n}.md"), &format!("- {n}\n"));
    }
    e.sync();
    std::fs::remove_dir_all(e.root().join("pages")).unwrap();
    let err = sync(e.root(), &e.remote(), &e.options(false)).unwrap_err();
    assert!(matches!(err, Error::MassDelete(14)), "{err}");
    assert_eq!(
        e.dav.files("Notes/g").len(),
        14,
        "nothing was deleted on the server"
    );

    // the other way: the server lost everything
    let e2 = Env::new();
    for n in 0..14 {
        e2.write(&format!("pages/p{n}.md"), "- x\n");
    }
    e2.sync();
    e2.dav.delete_path("Notes/g");
    assert!(matches!(
        sync(e2.root(), &e2.remote(), &e2.options(false)).unwrap_err(),
        Error::MassDelete(14)
    ));
    assert_eq!(
        std::fs::read_dir(e2.root().join("pages")).unwrap().count(),
        14
    );

    // allowed on purpose
    let mut o = e.options(false);
    o.allow_mass_delete = true;
    let r = sync(e.root(), &e.remote(), &o).unwrap();
    assert_eq!(r.deleted_remote.len(), 14);
}

#[test]
fn folders_with_an_unchanged_etag_are_not_listed_again() {
    let e = Env::new();
    for d in ["pages", "journals", "assets"] {
        e.write(&format!("{d}/x.md"), "- x\n");
    }
    let r = sync(e.root(), &e.remote(), &e.options(true)).unwrap();
    assert!(r.complete);
    // nothing changed: one question about the root, nothing else
    e.dav.clear_requests();
    let r = sync(e.root(), &e.remote(), &e.options(true)).unwrap();
    assert!(r.complete && !r.changed());
    assert_eq!(e.dav.requests(), ["PROPFIND Notes/g"]);
    // a change on the server in one folder: the root and that folder are listed, the others are not
    e.dav.put_file("Notes/g/pages/y.md", b"- y\n");
    e.dav.clear_requests();
    let r = sync(e.root(), &e.remote(), &e.options(true)).unwrap();
    assert_eq!(r.pulled, ["pages/y.md"]);
    let listed: Vec<String> = e
        .dav
        .requests()
        .into_iter()
        .filter(|q| q.starts_with("PROPFIND"))
        .collect();
    assert!(
        listed.contains(&"PROPFIND Notes/g/pages".to_string()),
        "{listed:?}"
    );
    assert!(
        !listed.contains(&"PROPFIND Notes/g/journals".to_string()),
        "{listed:?}"
    );
    assert_eq!(e.read("pages/y.md"), "- y\n");
}

#[test]
fn the_connection_test_learns_what_the_server_guarantees() {
    let e = Env::new();
    let p = probe(&e.remote()).unwrap();
    assert!(p.trusts_folder_etags);
    // the test leaves only the folder itself
    assert!(e.dav.files("Notes/g").is_empty());

    let e = Env::new();
    e.dav.set_propagate(false);
    assert!(!probe(&e.remote()).unwrap().trusts_folder_etags);

    // a server that ignores conditions would lose edits: refused
    let e = Env::new();
    e.dav.set_conditions(false);
    let err = probe(&e.remote()).unwrap_err();
    assert!(matches!(err, Error::Server(_)), "{err}");
}

#[test]
fn login_and_server_problems_are_told_apart() {
    let e = Env::new();
    let bad = WebDavRemote::new(&e.dav.folder_url("Notes/g"), &e.dav.user, "wrong").unwrap();
    let err = sync(e.root(), &bad, &e.options(false)).unwrap_err();
    assert_eq!(err.code(), "auth");

    e.write("pages/a.md", "- a\n");
    e.dav.set_down(true);
    let err = sync(e.root(), &e.remote(), &e.options(false)).unwrap_err();
    assert_eq!(err.code(), "server");
    e.dav.set_down(false);
    assert!(e.sync().complete, "the next sync picks up where it stopped");
    assert!(e.remote_text("pages/a.md").is_some());

    // a port nobody listens on
    let gone = WebDavRemote::new("http://127.0.0.1:9/dav", "u", "p").unwrap();
    assert_eq!(gone.list("").unwrap_err().code(), "offline");
}

#[test]
fn plain_http_is_for_this_computer_and_the_local_network_only() {
    for ok in [
        "https://cloud.example.org/remote.php/dav/files/me/Notes",
        "http://localhost:8080/remote.php/dav/files/me",
        "http://127.0.0.1/dav",
        "http://192.168.1.20:8080/dav",
        "http://10.0.0.5/dav",
        "http://172.16.0.1/dav",
        "http://nas.local/dav",
        "http://[::1]:8080/dav",
    ] {
        assert!(check_url(ok).is_ok(), "{ok}");
    }
    for bad in [
        "http://cloud.example.org/dav",
        "http://8.8.8.8/dav",
        "http://172.32.0.1/dav",
        "http://localhost.evil.org/dav",
        "http://user@evil.org/dav",
        "ftp://localhost/dav",
        "localhost:8080",
    ] {
        assert!(check_url(bad).is_err(), "{bad}");
    }
}

#[allow(dead_code)]
fn unused(_: PathBuf) {}

/// The laptop and the phone share one folder on the server and take turns (SY-1): the whole story in one go. Runs
/// against the in-memory server, and against a real one with `NOTATNIK_WEBDAV_URL`, `_USER`, `_PASSWORD` (see the
/// ignored test below).
fn two_devices(url: &str, user: &str, password: &str) {
    let remote = || WebDavRemote::new(url, user, password).unwrap();
    let laptop = tempfile::tempdir().unwrap();
    let phone = tempfile::tempdir().unwrap();
    let (ls, ps) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let go = |root: &Path, st: &tempfile::TempDir| {
        let mut o = Options::new(st.path().join("state.json"));
        o.trust_folder_etags = probe(&remote()).unwrap().trusts_folder_etags;
        let r = sync(root, &remote(), &o).unwrap();
        if std::env::var_os("SYNC_DEBUG").is_some() {
            eprintln!("{} {r:?}", root.display());
        }
        r
    };
    let write = |root: &Path, rel: &str, text: &str| {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    };
    let read = |root: &Path, rel: &str| std::fs::read_to_string(root.join(rel)).unwrap();

    // at home: write notes, sync
    write(laptop.path(), "journals/2026/2026-10-17.md", "- morning\n");
    write(laptop.path(), "pages/shopping.md", "- milk\n- bread\n");
    write(laptop.path(), ".notatnik/config.toml", "[graph]\n");
    assert!(go(laptop.path(), &ls).complete);
    // the phone attaches and sees everything
    let r = go(phone.path(), &ps);
    assert!(r.complete && r.pulled.len() == 3, "{r:?}");
    assert_eq!(read(phone.path(), "pages/shopping.md"), "- milk\n- bread\n");
    // on the way: a note on the phone, with a picture
    write(
        phone.path(),
        "journals/2026/2026-10-17.md",
        "- morning\n- on the bus: call Jan\n",
    );
    std::fs::create_dir_all(phone.path().join("assets/2026/10")).unwrap();
    std::fs::write(
        phone.path().join("assets/2026/10/pasted-aabbccdd.png"),
        [0x89, b'P', b'N', b'G', 1, 2, 3],
    )
    .unwrap();
    assert!(go(phone.path(), &ps).complete);
    // back home: the laptop gets it, edits another page, syncs again
    let r = go(laptop.path(), &ls);
    assert_eq!(r.pulled.len(), 2, "{r:?}");
    assert_eq!(
        read(laptop.path(), "journals/2026/2026-10-17.md"),
        "- morning\n- on the bus: call Jan\n"
    );
    write(
        laptop.path(),
        "pages/shopping.md",
        "- milk\n- bread\n- eggs\n",
    );
    assert!(go(laptop.path(), &ls).complete);
    assert_eq!(
        read(phone.path(), "pages/shopping.md"),
        "- milk\n- bread\n",
        "the phone is behind until it syncs"
    );
    assert!(go(phone.path(), &ps).complete);
    assert_eq!(
        read(phone.path(), "pages/shopping.md"),
        "- milk\n- bread\n- eggs\n"
    );
    // both edit the same page before syncing: nothing is lost
    write(
        laptop.path(),
        "pages/shopping.md",
        "- milk\n- bread\n- eggs\n- tea\n",
    );
    write(
        phone.path(),
        "pages/shopping.md",
        "- milk\n- bread\n- eggs\n- coffee\n",
    );
    assert!(go(laptop.path(), &ls).complete);
    let r = go(phone.path(), &ps);
    assert_eq!(r.conflicts, ["pages/shopping.md"], "{r:?}");
    assert_eq!(
        read(phone.path(), "pages/shopping.md"),
        "- milk\n- bread\n- eggs\n- tea\n"
    );
    let copy = &r.conflict_files[0];
    assert!(read(phone.path(), copy).contains("coffee"));
    assert!(go(laptop.path(), &ls).complete);
    assert!(
        read(laptop.path(), copy).contains("coffee"),
        "the copy reaches the other device too"
    );
    // a page is deleted on the laptop and gone from the phone after its next sync
    std::fs::remove_file(laptop.path().join("pages/shopping.md")).unwrap();
    assert!(go(laptop.path(), &ls).complete);
    assert!(go(phone.path(), &ps).complete);
    assert!(!phone.path().join("pages/shopping.md").exists());
    assert!(!go(phone.path(), &ps).changed());
    assert!(!go(laptop.path(), &ls).changed());
}

#[test]
fn laptop_and_phone_take_turns() {
    let dav = FakeDav::start();
    two_devices(&dav.folder_url("Notes/g"), &dav.user, &dav.password);
}

/// Against a real server: `NOTATNIK_WEBDAV_URL=http://localhost:8080/remote.php/dav/files/me NOTATNIK_WEBDAV_USER=me
/// NOTATNIK_WEBDAV_PASSWORD=… cargo test -p notatnik-sync --test sync -- --ignored`. Uses and removes the folder
/// `Notes/notatnik-live-test-<process id>` below the URL.
#[test]
#[ignore = "needs a WebDAV server and its login in the environment"]
fn laptop_and_phone_take_turns_on_a_real_server() {
    let (Ok(url), Ok(user), Ok(password)) = (
        std::env::var("NOTATNIK_WEBDAV_URL"),
        std::env::var("NOTATNIK_WEBDAV_USER"),
        std::env::var("NOTATNIK_WEBDAV_PASSWORD"),
    ) else {
        panic!("set NOTATNIK_WEBDAV_URL, NOTATNIK_WEBDAV_USER and NOTATNIK_WEBDAV_PASSWORD");
    };
    let folder = format!(
        "{}/Notes/notatnik-live-test-{}",
        url.trim_end_matches('/'),
        std::process::id()
    );
    two_devices(&folder, &user, &password);
    WebDavRemote::new(&folder, &user, &password)
        .unwrap()
        .delete("", None)
        .unwrap();
}

#[test]
fn a_folder_address_is_made_from_what_a_person_types() {
    assert_eq!(
        folder_url("https://cloud.example.org/", "me", "Notes/my graph").unwrap(),
        "https://cloud.example.org/remote.php/dav/files/me/Notes/my%20graph"
    );
    // the full WebDAV address as Nextcloud shows it in its settings
    assert_eq!(
        folder_url(
            "http://localhost:8080/remote.php/dav/files/notatkiA",
            "notatkiA",
            "/Notes/g/"
        )
        .unwrap(),
        "http://localhost:8080/remote.php/dav/files/notatkiA/Notes/g"
    );
    assert_eq!(
        folder_url("https://x.org", "me", "").unwrap(),
        "https://x.org/remote.php/dav/files/me"
    );
    assert!(folder_url("https://x.org", "me", "../etc").is_err());
    assert!(folder_url("http://cloud.example.org", "me", "Notes").is_err());
    assert!(folder_url("", "me", "Notes").is_err());
    assert!(folder_url("https://x.org", "", "Notes").is_err());
}

#[test]
fn a_server_that_mounts_the_files_like_nextcloud_works_the_same() {
    let dav = FakeDav::start();
    let url = folder_url(&dav.server, &dav.user, "Notes/graph").unwrap();
    let remote = WebDavRemote::new(&url, &dav.user, &dav.password).unwrap();
    let root = tempfile::tempdir().unwrap();
    let st = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(root.path().join("pages")).unwrap();
    std::fs::write(root.path().join("pages/a.md"), "- a\n").unwrap();
    let mut o = Options::new(st.path().join("s.json"));
    o.trust_folder_etags = true;
    assert!(sync(root.path(), &remote, &o).unwrap().complete);
    let listing = notatnik_sync::Remote::list(&remote, "").unwrap().unwrap();
    assert!(
        !listing.etag.is_empty() && listing.entries.len() == 1,
        "{listing:?}"
    );
    dav.put_file("Notes/graph/pages/b.md", b"- b\n");
    let r = sync(root.path(), &remote, &o).unwrap();
    assert_eq!(r.pulled, ["pages/b.md"]);
}
