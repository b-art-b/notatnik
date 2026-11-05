//! NX1, NX2: documents of the graph's Nextcloud (NA-1 to NA-8) against the in-memory server of the sync tests.

use std::path::Path;
use std::sync::Arc;

use chrono::NaiveDate;
use notatnik_app::{ApiError, AppOptions, AppState, dispatch};
use notatnik_graph::Dirs;
use notatnik_sync::fakedav::FakeDav;
use serde_json::{Value, json};

struct T {
    tmp: tempfile::TempDir,
    app: AppState,
    dav: FakeDav,
    root: std::path::PathBuf,
}

fn synced_with(limit: Option<u64>) -> T {
    let tmp = tempfile::tempdir().unwrap();
    let today = NaiveDate::from_ymd_opt(2026, 10, 3).unwrap();
    let mut app = AppState::new(AppOptions {
        dirs: Dirs::under(&tmp.path().join("device")),
        emit: Arc::new(|_, _| {}),
        today: Arc::new(move || today),
        watch: false,
    });
    if let Some(l) = limit {
        app = app.with_remote_cache_limit(l);
    }
    let dav = FakeDav::start();
    let info = dispatch(
        &app,
        "create_remote_graph",
        json!({"server": dav.server, "user": dav.user, "password": dav.password, "folder": "Notes/graph", "attach": false}),
    )
    .unwrap();
    let root = std::path::PathBuf::from(info["root"].as_str().unwrap());
    T {
        tmp,
        app,
        dav,
        root,
    }
}

fn synced() -> T {
    synced_with(None)
}

impl T {
    fn call(&self, name: &str, args: Value) -> Result<Value, ApiError> {
        dispatch(&self.app, name, args)
    }
    fn ok(&self, name: &str, args: Value) -> Value {
        self.call(name, args)
            .unwrap_or_else(|e| panic!("{name}: {e}"))
    }
    fn link(&self, path: &str) -> String {
        let id = self.dav.file_id(path).unwrap();
        format!("{}/f/{id}", self.dav.server)
    }
    fn open(&self, url: &str) -> Value {
        self.ok("remote_open", json!({"url": url}))
    }
    /// The files in the device's cache of downloaded documents.
    fn cached(&self) -> Vec<String> {
        let mut out = Vec::new();
        fn walk(dir: &Path, out: &mut Vec<String>) {
            if let Ok(rd) = std::fs::read_dir(dir) {
                for e in rd.flatten() {
                    if e.path().is_dir() {
                        walk(&e.path(), out);
                    } else if e.file_name() != "meta.json" {
                        out.push(e.path().to_string_lossy().into_owned());
                    }
                }
            }
        }
        for g in std::fs::read_dir(self.tmp.path().join("device/data/graphs"))
            .into_iter()
            .flatten()
            .flatten()
        {
            walk(&g.path().join("remote"), &mut out);
        }
        out.sort();
        out
    }
}

const PDF: &[u8] = b"%PDF-1.4\n% a document\n";

#[test]
fn a_link_by_number_opens_a_pdf_into_the_cache_and_serves_it() {
    let t = synced();
    t.dav.put_file("Docs/Report.pdf", PDF);
    let r = t.open(&t.link("Docs/Report.pdf"));
    assert_eq!(r["opened"], true, "{r}");
    assert_eq!(r["kind"], "pdf");
    assert_eq!(r["name"], "Report.pdf");
    assert_eq!(r["downloaded"], true);
    assert_eq!(r["offline"], false);
    assert_eq!(r["address"], t.link("Docs/Report.pdf"));
    let path = r["path"].as_str().unwrap();
    assert!(
        path.starts_with("cache/remote/") && path.ends_with(".pdf"),
        "{path}"
    );
    // the viewers read it through the asset route
    assert_eq!(t.app.read_asset(path).unwrap(), PDF);
    assert_eq!(t.app.read_asset(&format!("../{path}")).unwrap(), PDF);
    // nothing in the graph
    assert!(!t.root.join("assets").exists());
    assert!(!t.root.join("cache").exists());
}

#[test]
fn a_link_by_path_gives_the_same_file_and_address() {
    let t = synced();
    t.dav.put_file("Docs/My Report.pdf", PDF);
    let by_id = t.open(&t.link("Docs/My Report.pdf"));
    let by_path = t.open(&format!(
        "{}/remote.php/dav/files/{}/Docs/My%20Report.pdf",
        t.dav.server, t.dav.user
    ));
    assert_eq!(by_path["address"], by_id["address"]);
    assert_eq!(by_path["path"], by_id["path"]);
    assert_eq!(by_path["downloaded"], false);
}

#[test]
fn the_second_open_asks_for_the_etag_only_and_a_changed_file_is_fetched_again() {
    let t = synced();
    t.dav.put_file("Docs/Report.pdf", PDF);
    let link = t.link("Docs/Report.pdf");
    let first = t.open(&link);
    t.dav.clear_requests();
    let second = t.open(&link);
    assert_eq!(second["downloaded"], false);
    assert_eq!(second["path"], first["path"]);
    assert!(
        t.dav.requests().iter().all(|r| !r.starts_with("GET")),
        "{:?}",
        t.dav.requests()
    );
    assert_eq!(second["fetched"], "2026-10-03");
    // the file changes on the server
    t.dav
        .put_file("Docs/Report.pdf", b"%PDF-1.4\n% version two\n");
    let third = t.open(&link);
    assert_eq!(third["downloaded"], true);
    assert_ne!(third["path"], first["path"]);
    assert_eq!(
        t.app.read_asset(third["path"].as_str().unwrap()).unwrap(),
        b"%PDF-1.4\n% version two\n"
    );
    // the old copy is gone: one file per document
    assert_eq!(t.cached().len(), 1, "{:?}", t.cached());
    assert!(t.app.read_asset(first["path"].as_str().unwrap()).is_err());
}

#[test]
fn a_renamed_or_moved_file_is_still_found_by_its_number() {
    let t = synced();
    t.dav.put_file("Docs/Report.pdf", PDF);
    let link = t.link("Docs/Report.pdf");
    let before = t.open(&link);
    t.dav.put_file("Archive/keep", b"");
    t.dav.move_path("Docs/Report.pdf", "Archive/Final.pdf");
    let after = t.open(&link);
    assert_eq!(after["opened"], true);
    assert_eq!(after["name"], "Final.pdf");
    assert_eq!(after["address"], before["address"]);
    // the same bytes under the same number: the copy is current
    assert_eq!(after["downloaded"], false);
}

#[test]
fn what_is_not_a_document_of_this_server_is_not_opened_by_the_app() {
    let t = synced();
    t.dav.put_file("Docs/Letter.docx", b"PK");
    t.dav.put_file("Docs/Book.epub", b"PK");
    let docx = t.open(&t.link("Docs/Letter.docx"));
    assert_eq!(
        (docx["opened"].clone(), docx["reason"].clone()),
        (json!(false), json!("other_type"))
    );
    assert_eq!(docx["name"], "Letter.docx");
    assert!(t.cached().is_empty());
    for other in [
        "https://example.org/f/12345",
        "https://example.org/some/page",
        &format!("{}.evil.org/f/1", t.dav.server),
        &format!("{}/apps/files/?dir=/x", t.dav.server),
        &format!("{}/remote.php/dav/files/someone-else/a.pdf", t.dav.server),
    ] {
        let r = t.open(other);
        assert_eq!(
            (r["opened"].clone(), r["reason"].clone()),
            (json!(false), json!("other_server")),
            "{other}"
        );
    }
    assert_eq!(t.open(&t.link("Docs/Book.epub"))["kind"], "epub");
    // nothing but GETs of documents and the questions about them reached the server for the others
}

#[test]
fn a_graph_without_a_server_opens_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    let graph = tmp.path().join("graph");
    std::fs::create_dir_all(&graph).unwrap();
    let today = NaiveDate::from_ymd_opt(2026, 10, 3).unwrap();
    let app = AppState::new(AppOptions {
        dirs: Dirs::under(&tmp.path().join("device")),
        emit: Arc::new(|_, _| {}),
        today: Arc::new(move || today),
        watch: false,
    });
    dispatch(
        &app,
        "open_graph",
        json!({"path": graph.to_str().unwrap(), "create": true}),
    )
    .unwrap();
    let r = dispatch(
        &app,
        "remote_open",
        json!({"url": "https://cloud.example.org/f/1"}),
    )
    .unwrap();
    assert_eq!(
        (r["opened"].clone(), r["reason"].clone()),
        (json!(false), json!("not_synced"))
    );
}

#[test]
fn the_server_out_of_reach_opens_the_copy_of_an_earlier_open_and_says_so() {
    let t = synced();
    t.dav.put_file("Docs/Report.pdf", PDF);
    let link = t.link("Docs/Report.pdf");
    let first = t.open(&link);
    let T { app, dav, tmp, .. } = t;
    drop(dav); // the server is gone
    let r = dispatch(&app, "remote_open", json!({"url": link})).unwrap();
    assert_eq!(r["opened"], true, "{r}");
    assert_eq!(r["offline"], true);
    assert_eq!(r["path"], first["path"]);
    assert_eq!(r["name"], "Report.pdf");
    assert_eq!(r["fetched"], "2026-10-03");
    // a document that was never opened here cannot be had
    let other = link.replace("/f/", "/f/9");
    let e = dispatch(&app, "remote_open", json!({"url": other})).unwrap_err();
    assert_eq!(e.code, "offline", "{e}");
    drop(tmp);
}

#[test]
fn a_file_over_100_mb_is_refused_before_any_download() {
    let t = synced();
    t.dav
        .put_file("Docs/Huge.pdf", &vec![b'x'; 100 * 1024 * 1024 + 1]);
    t.dav.clear_requests();
    let e = t
        .call("remote_open", json!({"url": t.link("Docs/Huge.pdf")}))
        .unwrap_err();
    assert_eq!(e.code, "too_large", "{e}");
    assert!(t.dav.requests().iter().all(|r| !r.starts_with("GET")));
    assert!(t.cached().is_empty());
}

#[test]
fn a_wrong_password_a_missing_file_and_a_server_error_are_told_without_the_path() {
    let t = synced();
    t.dav.put_file("Docs/Secret Plans.pdf", PDF);
    let link = t.link("Docs/Secret Plans.pdf");
    // missing: a number nobody has
    let e = t
        .call(
            "remote_open",
            json!({"url": format!("{}/f/99999", t.dav.server)}),
        )
        .unwrap_err();
    assert_eq!(e.code, "not_found", "{e}");
    // a path that does not exist
    let e = t
        .call(
            "remote_open",
            json!({"url": format!("{}/remote.php/dav/files/{}/Docs/Nope Plans.pdf", t.dav.server, t.dav.user)}),
        )
        .unwrap_err();
    assert_eq!(e.code, "not_found");
    assert!(!e.message.contains("Nope"), "{e}");
    // 503
    t.dav.set_down(true);
    let e = t.call("remote_open", json!({"url": link})).unwrap_err();
    assert_eq!(e.code, "server");
    assert!(!e.message.contains("Secret"), "{e}");
    t.dav.set_down(false);
    // the login was changed on the server: the stored password is wrong now
    t.ok("sync_set_password", json!({"password": "wrong"}));
    let e = t.call("remote_open", json!({"url": link})).unwrap_err();
    assert_eq!(e.code, "auth", "{e}");
    assert!(!e.message.contains("Secret"), "{e}");
}

#[test]
fn the_cache_route_reaches_nothing_outside_the_cache() {
    let t = synced();
    t.dav.put_file("Docs/Report.pdf", PDF);
    let r = t.open(&t.link("Docs/Report.pdf"));
    let path = r["path"].as_str().unwrap().to_string();
    for bad in [
        "cache/remote/../../settings.toml",
        "cache/remote/x/../../../../device/config/settings.toml",
        "cache/remote//etc/passwd",
        "cache/remote/",
        "cache/remote",
        "cache/other/x.pdf",
        "cache/remote/a\\b.pdf",
    ] {
        assert!(t.app.read_asset(bad).is_err(), "{bad}");
        assert!(t.app.asset_file(bad).is_err(), "{bad}");
    }
    // a link inside the cache that points out of it
    #[cfg(unix)]
    {
        let secret = t.tmp.path().join("secret.pdf");
        std::fs::write(&secret, b"%PDF secret").unwrap();
        let dir = t.tmp.path().join("device/data/graphs");
        let key_dir = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .map(|g| g.path().join("remote"))
            .find(|p| p.exists())
            .unwrap()
            .read_dir()
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .find(|p| p.is_dir())
            .unwrap();
        std::os::unix::fs::symlink(&secret, key_dir.join("evil.pdf")).unwrap();
        let rel = format!(
            "cache/remote/{}/evil.pdf",
            key_dir.file_name().unwrap().to_string_lossy()
        );
        assert!(t.app.read_asset(&rel).is_err());
    }
    // and the real one still works
    assert_eq!(t.app.read_asset(&path).unwrap(), PDF);
}

#[test]
fn the_cache_is_cleaned_oldest_first_above_the_limit_and_never_the_file_just_opened() {
    let t = synced_with(Some(3000));
    for (i, name) in ["A", "B", "C"].iter().enumerate() {
        t.dav
            .put_file(&format!("Docs/{name}.pdf"), &vec![b'a' + i as u8; 1500]);
    }
    let a = t.open(&t.link("Docs/A.pdf"));
    std::thread::sleep(std::time::Duration::from_millis(1100));
    t.open(&t.link("Docs/B.pdf"));
    std::thread::sleep(std::time::Duration::from_millis(1100));
    let c = t.open(&t.link("Docs/C.pdf"));
    // A is the oldest: gone; B and C stay (3000 bytes)
    assert!(t.app.read_asset(a["path"].as_str().unwrap()).is_err());
    assert!(t.app.read_asset(c["path"].as_str().unwrap()).is_ok());
    assert_eq!(t.cached().len(), 2, "{:?}", t.cached());
    // opening A again fetches it again and now B, the oldest, goes
    let a2 = t.open(&t.link("Docs/A.pdf"));
    assert_eq!(a2["downloaded"], true);
    assert!(t.app.read_asset(a2["path"].as_str().unwrap()).is_ok());
    assert_eq!(t.cached().len(), 2);
    // clearing frees the rest
    let cleared = t.ok("remote_cache_clear", json!({}));
    assert!(cleared["freed"].as_u64().unwrap() >= 3000);
    assert!(t.cached().is_empty());
}

// ------------------------------------------------------------------------------------------------ the sidecar

#[test]
fn the_sidecar_of_a_remote_document_has_its_address_and_follows_the_file_when_it_changes() {
    let t = synced();
    t.dav.put_file("Docs/Report.pdf", PDF);
    let link = t.link("Docs/Report.pdf");
    let r = t.open(&link);
    let args = json!({"path": r["path"], "origin": r["address"]});
    // looking makes no page
    let looked = t.ok(
        "annotation_page",
        json!({"path": r["path"], "origin": r["address"], "create": false}),
    );
    assert_eq!(looked["page"], "");
    let side = t.ok("annotation_page", args.clone());
    assert_eq!(side["created"], true);
    assert_eq!(side["page"], "hls/Report");
    assert_eq!(side["file"], link);
    let text = std::fs::read_to_string(t.root.join("pages/hls/Report.md")).unwrap();
    assert!(text.contains(&format!("file: {link}")), "{text}");
    assert!(
        text.contains(&format!("sha: {}", side["sha"].as_str().unwrap())),
        "{text}"
    );
    assert!(text.contains("archived: true"));
    // again: the same page
    let again = t.ok("annotation_page", args);
    assert_eq!(
        (again["page"].clone(), again["created"].clone()),
        (json!("hls/Report"), json!(false))
    );

    // the file changed on the server: the same sidecar, with the new hash
    t.dav
        .put_file("Docs/Report.pdf", b"%PDF-1.4\n% version two\n");
    let r2 = t.open(&link);
    let after = t.ok(
        "annotation_page",
        json!({"path": r2["path"], "origin": r2["address"]}),
    );
    assert_eq!(
        (after["page"].clone(), after["created"].clone()),
        (json!("hls/Report"), json!(false))
    );
    assert_ne!(after["sha"], side["sha"]);
    let text = std::fs::read_to_string(t.root.join("pages/hls/Report.md")).unwrap();
    assert!(
        text.contains(&format!("sha: {}", after["sha"].as_str().unwrap())),
        "{text}"
    );
}

#[test]
fn keeping_a_copy_points_the_sidecar_at_it_and_the_old_link_finds_the_same_sidecar() {
    let t = synced();
    t.dav.put_file("Docs/Report.pdf", PDF);
    let link = t.link("Docs/Report.pdf");
    let r = t.open(&link);
    let side = t.ok(
        "annotation_page",
        json!({"path": r["path"], "origin": r["address"]}),
    );
    let kept = t.ok(
        "keep_document",
        json!({"path": r["path"], "address": r["address"], "page": "inbox"}),
    );
    let path = kept["path"].as_str().unwrap();
    assert!(
        path.starts_with("assets/2026/10/Report-") && path.ends_with(".pdf"),
        "{path}"
    );
    assert_eq!(std::fs::read(t.root.join(path)).unwrap(), PDF);
    assert_eq!(kept["sidecar_moved"], true);
    assert!(
        kept["markdown"]
            .as_str()
            .unwrap()
            .starts_with("[Report.pdf](../"),
        "{kept}"
    );
    let text = std::fs::read_to_string(t.root.join("pages/hls/Report.md")).unwrap();
    assert!(text.contains(&format!("file: {path}")), "{text}");

    // opened by the local path: the same sidecar
    let local = t.ok("annotation_page", json!({"path": path}));
    assert_eq!(
        (local["page"].clone(), local["created"].clone()),
        (json!("hls/Report"), json!(false))
    );
    // opened by the old link: the same sidecar, not a second one, and it keeps pointing at the copy
    let remote_again = t.ok(
        "annotation_page",
        json!({"path": r["path"], "origin": r["address"]}),
    );
    assert_eq!(remote_again["page"], side["page"]);
    assert_eq!(remote_again["created"], false);
    let text = std::fs::read_to_string(t.root.join("pages/hls/Report.md")).unwrap();
    assert!(text.contains(&format!("file: {path}")), "{text}");
    assert_eq!(
        std::fs::read_dir(t.root.join("pages/hls")).unwrap().count(),
        1
    );
}

#[test]
fn keeping_a_copy_of_a_document_without_highlights_just_copies_it() {
    let t = synced();
    t.dav
        .put_file("Docs/Book.epub", b"PK\x03\x04 not really a book");
    let r = t.open(&t.link("Docs/Book.epub"));
    let kept = t.ok(
        "keep_document",
        json!({"path": r["path"], "address": r["address"], "page": "inbox"}),
    );
    assert_eq!(kept["sidecar_moved"], false);
    assert!(kept["path"].as_str().unwrap().ends_with(".epub"));
    assert!(!t.root.join("pages/hls").exists());
}

#[test]
fn the_sidecar_calls_refuse_what_is_not_a_cached_document() {
    let t = synced();
    for path in ["assets/x.pdf", "cache/remote/../x", "pages/inbox.md"] {
        let e = t
            .call(
                "annotation_page",
                json!({"path": path, "origin": "https://x.example/f/1"}),
            )
            .unwrap_err();
        assert!(
            ["invalid", "not_found"].contains(&e.code.as_str()),
            "{path}: {e}"
        );
    }
    t.dav.put_file("Docs/Report.pdf", PDF);
    let r = t.open(&t.link("Docs/Report.pdf"));
    let e = t
        .call(
            "annotation_page",
            json!({"path": r["path"], "origin": "javascript:alert(1)"}),
        )
        .unwrap_err();
    assert_eq!(e.code, "invalid");
}

// ------------------------------------------------------------------------------------------------ the log

static LOG: std::sync::OnceLock<std::sync::Mutex<Vec<(log::Level, String)>>> =
    std::sync::OnceLock::new();

struct Listen;
impl log::Log for Listen {
    fn enabled(&self, _: &log::Metadata) -> bool {
        true
    }
    fn log(&self, r: &log::Record) {
        LOG.get_or_init(Default::default)
            .lock()
            .unwrap()
            .push((r.level(), format!("{}", r.args())));
    }
    fn flush(&self) {}
}

#[test]
fn the_address_the_path_and_the_name_of_a_document_are_not_in_the_log_above_debug() {
    static L: Listen = Listen;
    let _ = log::set_logger(&L);
    log::set_max_level(log::LevelFilter::Trace);
    let t = synced();
    t.dav.put_file("Zebra Folder/Zebra Plans.pdf", PDF);
    let link = t.link("Zebra Folder/Zebra Plans.pdf");
    let r = t.open(&link);
    t.open(&link);
    t.ok(
        "annotation_page",
        json!({"path": r["path"], "origin": r["address"]}),
    );
    let _ = t.call(
        "remote_open",
        json!({"url": format!("{}/f/424242", t.dav.server)}),
    );
    t.dav.set_down(true);
    let _ = t.call("remote_open", json!({"url": link}));
    let host = t.dav.server.trim_start_matches("http://").to_string();
    let lines = LOG.get_or_init(Default::default).lock().unwrap().clone();
    let mine: Vec<_> = lines
        .iter()
        .filter(|(lvl, m)| {
            // the sync of the graph itself names its server in its summary line; this is about the documents
            *lvl <= log::Level::Info
                && !m.starts_with("sync with")
                && (m.contains("Zebra")
                    || m.contains(&host)
                    || m.contains("/f/")
                    || m.contains("Plans"))
        })
        .collect();
    assert!(mine.is_empty(), "{mine:?}");
    // the document was logged by its number and size
    assert!(
        lines
            .iter()
            .any(|(lvl, m)| *lvl == log::Level::Info && m.contains("opened a Nextcloud document")),
        "{lines:?}"
    );
}
