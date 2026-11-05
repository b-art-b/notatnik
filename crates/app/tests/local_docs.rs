//! LV1: PDFs and EPUBs of this computer read in place (LV-1 to LV-8).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::NaiveDate;
use notatnik_app::{ApiError, AppOptions, AppState, dispatch};
use notatnik_graph::Dirs;
use serde_json::{Value, json};

struct T {
    tmp: tempfile::TempDir,
    app: AppState,
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap().flatten() {
        let dest = to.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            copy_dir(&e.path(), &dest);
        } else {
            std::fs::copy(e.path(), dest).unwrap();
        }
    }
}

fn start(local: bool) -> T {
    let tmp = tempfile::tempdir().unwrap();
    let graph = tmp.path().join("graph");
    copy_dir(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/graphs/basic"),
        &graph,
    );
    let today = NaiveDate::from_ymd_opt(2026, 10, 3).unwrap();
    let mut app = AppState::new(AppOptions {
        dirs: Dirs::under(&tmp.path().join("device")),
        emit: Arc::new(|_, _| {}),
        today: Arc::new(move || today),
        watch: false,
    });
    if local {
        app = app.with_local_files();
    }
    let t = T { tmp, app };
    t.ok(
        "open_graph",
        json!({"path": graph.to_str().unwrap(), "create": false}),
    );
    t
}

impl T {
    fn call(&self, name: &str, args: Value) -> Result<Value, ApiError> {
        dispatch(&self.app, name, args)
    }
    fn ok(&self, name: &str, args: Value) -> Value {
        self.call(name, args)
            .unwrap_or_else(|e| panic!("{name}: {e}"))
    }
    fn graph(&self) -> PathBuf {
        self.tmp.path().join("graph")
    }
    /// A file outside the graph.
    fn file(&self, rel: &str, bytes: &[u8]) -> PathBuf {
        let p = self.tmp.path().join("elsewhere").join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, bytes).unwrap();
        p
    }
    fn open(&self, path: &Path) -> Value {
        self.ok("local_open", json!({"url": uri(path)}))
    }
}

/// `file:///…` for a path, spaces and other characters encoded the way a browser writes them.
fn uri(p: &Path) -> String {
    let s = p.to_string_lossy();
    let enc: String = s
        .bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || matches!(b, b'/' | b'-' | b'.' | b'_' | b'~') {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect();
    format!("file://{enc}")
}

const PDF: &[u8] = b"%PDF-1.4\n% a document\n";
const EPUB: &[u8] = b"PK\x03\x04 pretends to be a book";

#[test]
fn a_file_is_registered_in_place_and_served_by_its_token_only() {
    let t = start(true);
    let p = t.file("nc/Rozrachunki do luty 2024_0001.pdf", PDF);
    let r = t.open(&p);
    assert_eq!(r["kind"], "pdf");
    assert_eq!(r["name"], "Rozrachunki do luty 2024_0001.pdf");
    assert_eq!(r["size"], PDF.len());
    let address = r["address"].as_str().unwrap();
    assert!(address.starts_with("file:///"), "{address}");
    assert!(
        address.ends_with("/Rozrachunki%20do%20luty%202024_0001.pdf"),
        "{address}"
    );
    let path = r["path"].as_str().unwrap();
    assert!(path.starts_with("local/"), "{path}");
    assert_eq!(t.app.read_asset(path).unwrap(), PDF);
    assert_eq!(t.app.asset_file(path).unwrap(), p);
    assert_eq!(t.app.read_asset(&format!("../{path}")).unwrap(), PDF);
    // the same file, the same token: a viewer that is open keeps working
    assert_eq!(t.open(&p)["path"], r["path"]);
    // nothing was copied into the graph, and the file is as it was
    assert!(!t.graph().join("assets").exists());
    assert_eq!(std::fs::read(&p).unwrap(), PDF);
    // an EPUB, a localhost address and a page fragment
    let b = t.file("books/Zebra Book.epub", EPUB);
    let e = t.ok(
        "local_open",
        json!({"url": format!("file://localhost{}#page=3", uri(&b).trim_start_matches("file://"))}),
    );
    assert_eq!(e["kind"], "epub");
}

#[test]
fn a_token_is_the_only_way_to_a_file() {
    let t = start(true);
    let p = t.file("a.pdf", PDF);
    t.file("secret.txt", b"secret");
    let r = t.open(&p);
    let token = r["path"]
        .as_str()
        .unwrap()
        .trim_start_matches("local/")
        .to_string();
    let secret = t.tmp.path().join("elsewhere/secret.txt");
    for bad in [
        "local/".to_string(),
        "local/0000".to_string(),
        "local/../x".to_string(),
        "local//etc/passwd".to_string(),
        format!("local/{token}/../secret.txt"),
        format!("local/{}", secret.display()),
        p.to_string_lossy().into_owned(),
        "local/zz".to_string(),
        format!("local/{}x", token),
    ] {
        assert!(t.app.read_asset(&bad).is_err(), "{bad}");
        assert!(t.app.asset_file(&bad).is_err(), "{bad}");
    }
}

#[test]
fn what_is_not_a_document_is_refused() {
    let t = start(true);
    let not_pdf = t.file("fake.pdf", b"just some text, not a pdf");
    let not_epub = t.file("fake.epub", b"just some text, not a zip");
    let txt = t.file("a.txt", PDF);
    let dir = t.tmp.path().join("elsewhere/folder.pdf");
    std::fs::create_dir_all(&dir).unwrap();
    for (p, why) in [(&not_pdf, "PDF"), (&not_epub, "EPUB")] {
        let e = t.call("local_open", json!({"url": uri(p)})).unwrap_err();
        assert_eq!(e.code, "invalid");
        assert!(e.message.contains(why), "{e}");
    }
    for bad in [
        uri(&txt),
        uri(&dir),
        "file://server/share/a.pdf".to_string(),
        "file:///tmp/../etc/a.pdf".to_string(),
        "https://example.org/a.pdf".to_string(),
        "../a.pdf".to_string(),
    ] {
        let e = t.call("local_open", json!({"url": bad})).unwrap_err();
        assert_eq!(e.code, "invalid", "{bad}: {e}");
    }
    let missing = t.tmp.path().join("elsewhere/nope.pdf");
    let e = t
        .call("local_open", json!({"url": uri(&missing)}))
        .unwrap_err();
    assert_eq!(e.code, "not_found");
    assert!(!e.message.contains("nope"), "{e}");
}

#[test]
fn a_file_over_100_mb_is_refused_before_it_is_read_and_a_deleted_file_is_not_found() {
    let t = start(true);
    let big = t.file("big.pdf", PDF);
    std::fs::OpenOptions::new()
        .write(true)
        .open(&big)
        .unwrap()
        .set_len(100 * 1024 * 1024 + 1)
        .unwrap(); // a sparse file
    let e = t.call("local_open", json!({"url": uri(&big)})).unwrap_err();
    assert_eq!(e.code, "too_large", "{e}");

    let p = t.file("gone.pdf", PDF);
    let r = t.open(&p);
    std::fs::remove_file(&p).unwrap();
    let e = t.app.read_asset(r["path"].as_str().unwrap()).unwrap_err();
    assert_eq!(e.code, "not_found", "{e}");
}

#[test]
fn without_permission_nothing_is_read_and_the_reason_is_unsupported() {
    let t = start(false);
    let p = t.file("a.pdf", PDF);
    let e = t.call("local_open", json!({"url": uri(&p)})).unwrap_err();
    assert_eq!(e.code, "unsupported");
    assert!(t.app.read_asset("local/abcd").is_err());
}

// ------------------------------------------------------------------------------------------------ the sidecar

fn side(t: &T, r: &Value) -> Value {
    t.ok(
        "annotation_page",
        json!({"path": r["path"], "origin": r["address"]}),
    )
}

fn sidecar_text(t: &T, name: &str) -> String {
    std::fs::read_to_string(t.graph().join(format!("pages/{name}.md"))).unwrap()
}

#[test]
fn the_sidecar_has_the_address_is_found_again_and_follows_a_moved_file() {
    let t = start(true);
    let p = t.file("one/Report.pdf", PDF);
    let r = t.open(&p);
    // looking makes no page
    let looked = t.ok(
        "annotation_page",
        json!({"path": r["path"], "origin": r["address"], "create": false}),
    );
    assert_eq!(looked["page"], "");
    let s = side(&t, &r);
    assert_eq!(
        (s["page"].clone(), s["created"].clone()),
        (json!("hls/Report"), json!(true))
    );
    assert_eq!(s["file"], r["address"]);
    let text = sidecar_text(&t, "hls/Report");
    assert!(
        text.contains(&format!("file: {}", r["address"].as_str().unwrap())),
        "{text}"
    );
    assert!(text.contains("archived: true") && text.contains("kind: annotations"));
    let again = side(&t, &r);
    assert_eq!(
        (again["page"].clone(), again["created"].clone()),
        (json!("hls/Report"), json!(false))
    );

    // the file is moved and renamed: the same bytes, the same sidecar, and its `file` follows
    let to = t.tmp.path().join("elsewhere/two/Renamed report.pdf");
    std::fs::create_dir_all(to.parent().unwrap()).unwrap();
    std::fs::rename(&p, &to).unwrap();
    let r2 = t.open(&to);
    let moved = side(&t, &r2);
    assert_eq!(
        (moved["page"].clone(), moved["created"].clone()),
        (json!("hls/Report"), json!(false))
    );
    let text = sidecar_text(&t, "hls/Report");
    assert!(
        text.contains(&format!("file: {}", r2["address"].as_str().unwrap())),
        "{text}"
    );
    assert_eq!(
        std::fs::read_dir(t.graph().join("pages/hls"))
            .unwrap()
            .count(),
        1
    );
}

#[test]
fn a_copy_of_the_file_in_another_place_shares_the_sidecar_and_another_file_of_the_same_name_has_its_own()
 {
    let t = start(true);
    let a = t.file("one/Report.pdf", PDF);
    let ra = t.open(&a);
    let sa = side(&t, &ra);
    // the same bytes in a second place that is still there: found by the hash, `file` left alone
    let b = t.file("two/Report.pdf", PDF);
    let rb = t.open(&b);
    let sb = side(&t, &rb);
    assert_eq!(sb["page"], sa["page"]);
    assert_eq!(sb["created"], false);
    assert!(
        sidecar_text(&t, "hls/Report")
            .contains(&format!("file: {}", ra["address"].as_str().unwrap()))
    );
    // other bytes under the same name: its own sidecar, named apart
    let c = t.file("three/Report.pdf", b"%PDF-1.4\n% another one\n");
    let rc = t.open(&c);
    let sc = side(&t, &rc);
    assert_eq!(sc["created"], true);
    assert_ne!(sc["page"], sa["page"]);
    assert!(sc["page"].as_str().unwrap().starts_with("hls/Report-"));
}

#[test]
fn keeping_a_copy_points_the_sidecar_at_it_and_the_link_still_finds_it() {
    let t = start(true);
    let p = t.file("Report.pdf", PDF);
    let r = t.open(&p);
    let s = side(&t, &r);
    let kept = t.ok(
        "keep_document",
        json!({"path": r["path"], "address": r["address"], "page": "inbox"}),
    );
    let path = kept["path"].as_str().unwrap();
    assert!(
        path.starts_with("assets/2026/10/Report-") && path.ends_with(".pdf"),
        "{path}"
    );
    assert_eq!(std::fs::read(t.graph().join(path)).unwrap(), PDF);
    assert_eq!(kept["sidecar_moved"], true);
    assert!(sidecar_text(&t, "hls/Report").contains(&format!("file: {path}")));
    // the link to the file in place: the same sidecar, and it still points at the copy
    let again = side(&t, &r);
    assert_eq!(
        (again["page"].clone(), again["created"].clone()),
        (s["page"].clone(), json!(false))
    );
    assert!(sidecar_text(&t, "hls/Report").contains(&format!("file: {path}")));
    // the original is untouched
    assert_eq!(std::fs::read(&p).unwrap(), PDF);
}

#[test]
fn the_sidecar_calls_refuse_a_token_that_is_not_one_and_a_strange_origin() {
    let t = start(true);
    let p = t.file("a.pdf", PDF);
    let r = t.open(&p);
    let e = t
        .call(
            "annotation_page",
            json!({"path": "local/ffff", "origin": r["address"]}),
        )
        .unwrap_err();
    assert_eq!(e.code, "invalid");
    for origin in ["javascript:alert(1)", "ftp://x/a.pdf", "/etc/passwd"] {
        let e = t
            .call(
                "annotation_page",
                json!({"path": r["path"], "origin": origin}),
            )
            .unwrap_err();
        assert_eq!(e.code, "invalid", "{origin}");
    }
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
fn the_path_and_the_name_of_a_local_file_are_not_in_the_log_above_debug() {
    static L: Listen = Listen;
    let _ = log::set_logger(&L);
    log::set_max_level(log::LevelFilter::Trace);
    let t = start(true);
    let p = t.file("Quokka Folder/Quokka Plans.pdf", PDF);
    let r = t.open(&p);
    side(&t, &r);
    let _ = t.call(
        "local_open",
        json!({"url": uri(&p).replace("Plans", "Missing")}),
    );
    let _ = t.call("local_open", json!({"url": "file:///nowhere/Quokka.txt"}));
    t.ok(
        "keep_document",
        json!({"path": r["path"], "address": r["address"], "page": "inbox"}),
    );
    let lines = LOG.get_or_init(Default::default).lock().unwrap().clone();
    let leaked: Vec<_> = lines
        .iter()
        .filter(|(lvl, m)| {
            *lvl <= log::Level::Info && (m.contains("Quokka") || m.contains("elsewhere"))
        })
        .collect();
    assert!(leaked.is_empty(), "{leaked:?}");
    assert!(
        lines
            .iter()
            .any(|(l, m)| *l == log::Level::Info && m.contains("opened a file of this computer")),
        "{lines:?}"
    );
}
