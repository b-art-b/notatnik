//! W2: `clip_page` and its companions through `dispatch`, against pages served from this computer (the state is made
//! with `with_local_fetch`, as the dev server is with `--allow-local-fetch`). No test reaches the internet.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use chrono::NaiveDate;
use notatnik_app::{ApiError, AppOptions, AppState, dispatch};
use notatnik_graph::Dirs;
use serde_json::{Value, json};

// ------------------------------------------------------------------------------------------------ the app

struct T {
    tmp: tempfile::TempDir,
    app: AppState,
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        let dest = to.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            copy_dir(&e.path(), &dest);
        } else {
            std::fs::copy(e.path(), dest).unwrap();
        }
    }
}

fn start(local_fetch: bool) -> T {
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
    if local_fetch {
        app = app.with_local_fetch();
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
    fn graph(&self) -> std::path::PathBuf {
        self.tmp.path().join("graph")
    }
    fn clip(&self, url: &str) -> Result<Value, ApiError> {
        self.call("clip_page", json!({"url": url, "page": "inbox"}))
    }
    fn web_files(&self) -> Vec<String> {
        let mut v: Vec<String> = std::fs::read_dir(self.graph().join("assets/web"))
            .map(|d| {
                d.flatten()
                    .map(|e| e.file_name().to_string_lossy().into_owned())
                    .collect()
            })
            .unwrap_or_default();
        v.sort();
        v
    }
}

// ------------------------------------------------------------------------------------------------ a small server

#[derive(Clone)]
struct Route {
    content_type: &'static str,
    body: Vec<u8>,
    delay: Duration,
}

struct Server {
    addr: SocketAddr,
}

impl Server {
    fn start(routes: HashMap<String, Route>) -> Server {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let routes = routes.clone();
                std::thread::spawn(move || serve(stream, &routes));
            }
        });
        Server { addr }
    }
    fn url(&self, path: &str) -> String {
        format!("http://127.0.0.1:{}{path}", self.addr.port())
    }
}

fn serve(mut stream: TcpStream, routes: &HashMap<String, Route>) {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 1024];
    while !buf.windows(4).any(|w| w == b"\r\n\r\n") {
        match stream.read(&mut chunk) {
            Ok(0) | Err(_) => return,
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
        }
    }
    let head = String::from_utf8_lossy(&buf).to_string();
    let path = head
        .split_whitespace()
        .nth(1)
        .unwrap_or("/")
        .split('?')
        .next()
        .unwrap_or("/")
        .to_string();
    match routes.get(&path) {
        Some(r) => {
            std::thread::sleep(r.delay);
            let _ = write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                r.content_type,
                r.body.len()
            );
            let _ = stream.write_all(&r.body);
        }
        None => {
            let _ = write!(
                stream,
                "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            );
        }
    }
}

const PNG: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4,
    0x89, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0xfc, 0xcf, 0xc0, 0xf0,
    0x1f, 0x00, 0x05, 0x85, 0x02, 0x80, 0x84, 0xa9, 0x8c, 0x21, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45,
    0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
];

fn article(title: &str, pics: &[&str], extra: &str) -> Route {
    let mut body = format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><title>{title}</title></head><body><article><h1>{title}</h1>"
    );
    for i in 0..8 {
        body.push_str(&format!("<p>Paragraph {i}: the keeper climbed the stairs one slow step at a time, counting each of them aloud so that the dark would not seem so large. {extra}</p>"));
    }
    for p in pics {
        body.push_str(&format!("<img src=\"{p}\" alt=\"picture\">"));
    }
    body.push_str("</article></body></html>");
    Route {
        content_type: "text/html",
        body: body.into_bytes(),
        delay: Duration::ZERO,
    }
}

fn picture(delay_ms: u64) -> Route {
    Route {
        content_type: "image/png",
        body: PNG.to_vec(),
        delay: Duration::from_millis(delay_ms),
    }
}

// ------------------------------------------------------------------------------------------------ a logger that listens

static LOG: OnceLock<Mutex<Vec<String>>> = OnceLock::new();

struct Listen;
impl log::Log for Listen {
    fn enabled(&self, _: &log::Metadata) -> bool {
        true
    }
    fn log(&self, r: &log::Record) {
        LOG.get_or_init(Default::default)
            .lock()
            .unwrap()
            .push(format!("{} {}", r.level(), r.args()));
    }
    fn flush(&self) {}
}

// ------------------------------------------------------------------------------------------------ tests

#[test]
fn a_page_is_clipped_into_assets_web_and_the_same_address_replaces_the_same_file() {
    let s = Server::start(HashMap::from([
        ("/a".to_string(), article("The Keeper", &["/p.png"], "")),
        ("/p.png".to_string(), picture(0)),
    ]));
    let t = start(true);
    let r = t.clip(&s.url("/a")).unwrap();
    let path = r["path"].as_str().unwrap().to_string();
    assert!(
        path.starts_with("assets/web/the-keeper-") && path.ends_with(".epub"),
        "{path}"
    );
    assert_eq!(r["title"], "The Keeper");
    assert_eq!(r["source"], s.url("/a"));
    assert_eq!(r["replaced"], false);
    // the link is relative to the page the clip was asked for (pages/inbox.md)
    assert_eq!(r["markdown"], format!("[The Keeper](../{path})"));
    let bytes = std::fs::read(t.graph().join(&path)).unwrap();
    assert_eq!(&bytes[..2], b"PK");
    assert!(bytes.windows(20).any(|w| w == b"application/epub+zip"));
    assert_eq!(
        t.web_files().len(),
        1,
        "no temp file is left: {:?}",
        t.web_files()
    );
    assert_eq!(
        t.ok("clip_existing", json!({"url": s.url("/a")}))["value"],
        path
    );
    assert_eq!(
        t.ok(
            "clip_existing",
            json!({"url": format!("{}#x", s.url("/a"))})
        )["value"],
        path,
        "a fragment is not another address"
    );
    assert_eq!(
        t.ok("clip_existing", json!({"url": s.url("/other")})),
        Value::Null
    );
}

#[test]
fn clipping_the_same_address_again_keeps_the_name_even_when_the_title_changed() {
    let page = Arc::new(Mutex::new(article("First Title", &[], "")));
    // a server whose page changes between two clips
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let served = page.clone();
    std::thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            let mut buf = [0u8; 2048];
            let _ = stream.read(&mut buf);
            let body = served.lock().unwrap().body.clone();
            let _ = write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = stream.write_all(&body);
        }
    });
    let url = format!("http://127.0.0.1:{}/page", addr.port());
    let t = start(true);
    let first = t.clip(&url).unwrap();
    assert_eq!(first["replaced"], false);
    let first_bytes = std::fs::read(t.graph().join(first["path"].as_str().unwrap())).unwrap();
    *page.lock().unwrap() = article("Second Title", &[], "Added.");
    let second = t.clip(&url).unwrap();
    assert_eq!(second["replaced"], true);
    assert_eq!(second["path"], first["path"], "the name stays");
    assert_eq!(second["title"], "Second Title");
    assert_eq!(t.web_files().len(), 1);
    let second_bytes = std::fs::read(t.graph().join(second["path"].as_str().unwrap())).unwrap();
    assert_ne!(first_bytes, second_bytes, "the file holds the new text");
}

#[test]
fn two_addresses_with_one_title_get_two_files_and_odd_titles_give_safe_names() {
    let s = Server::start(HashMap::from([
        ("/one".to_string(), article("Same Title", &[], "")),
        ("/two".to_string(), article("Same Title", &[], "")),
        (
            "/odd".to_string(),
            article("../../etc/passwd: <b>😀</b> \\ [x]", &[], ""),
        ),
    ]));
    let t = start(true);
    let a = t.clip(&s.url("/one")).unwrap();
    let b = t.clip(&s.url("/two")).unwrap();
    assert_ne!(a["path"], b["path"]);
    let odd = t.clip(&s.url("/odd")).unwrap();
    let path = odd["path"].as_str().unwrap();
    let ok = path
        .strip_prefix("assets/web/")
        .is_some_and(notatnik_graph::is_clip_name);
    assert!(ok, "{path}");
    // the label of the link has its brackets and backslash escaped, so the link stays one link
    let md = odd["markdown"].as_str().unwrap();
    assert!(
        md.contains("\\[x\\]") && md.contains("\\\\") && !md.contains('\n'),
        "{md}"
    );
    assert_eq!(t.web_files().len(), 3);
    // nothing was written outside assets/web
    assert!(!t.graph().join("etc").exists() && !t.graph().parent().unwrap().join("etc").exists());
}

#[test]
fn progress_arrives_in_order_and_a_cancel_stops_the_clip_and_leaves_no_file() {
    let pics: Vec<String> = (0..6).map(|i| format!("/p{i}.png")).collect();
    let mut routes = HashMap::from([(
        "/slow".to_string(),
        article(
            "Slow Pictures",
            &pics.iter().map(String::as_str).collect::<Vec<_>>(),
            "",
        ),
    )]);
    for p in &pics {
        routes.insert(p.clone(), picture(250));
    }
    let s = Server::start(routes);
    let t = start(true);
    std::thread::scope(|scope| {
        let job = scope.spawn(|| t.clip(&s.url("/slow")));
        let mut steps: Vec<String> = Vec::new();
        let mut cancelled = false;
        // a second clip at the same time is refused
        let mut refused_second = false;
        for _ in 0..200 {
            let p = t.ok("clip_progress", json!({}));
            if p["running"] == true {
                let step = p["step"].as_str().unwrap().to_string();
                if steps.last() != Some(&step) {
                    steps.push(step);
                }
                if !refused_second {
                    let e = t.clip(&s.url("/slow")).unwrap_err();
                    assert_eq!(e.code, "conflict", "{e}");
                    refused_second = true;
                }
                if p["step"] == "pictures" && p["done"].as_u64().unwrap() >= 2 && !cancelled {
                    t.ok("clip_cancel", json!({}));
                    cancelled = true;
                }
            } else if cancelled {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        let result = job.join().unwrap();
        let e = result.unwrap_err();
        assert_eq!(e.code, "cancelled", "{e}");
        assert_eq!(
            steps,
            ["fetching", "cleaning", "pictures"]
                .map(String::from)
                .to_vec()
                .into_iter()
                .filter(|s| steps.contains(s))
                .collect::<Vec<_>>()
        );
        assert!(steps.contains(&"pictures".to_string()), "{steps:?}");
        assert!(refused_second);
    });
    assert_eq!(t.ok("clip_progress", json!({}))["running"], false);
    assert!(
        t.web_files().is_empty(),
        "a cancelled clip leaves nothing: {:?}",
        t.web_files()
    );
    // and the next clip works
    assert!(t.clip(&s.url("/slow")).is_ok());
}

#[test]
fn a_slow_clip_does_not_block_the_other_commands() {
    let s = Server::start(HashMap::from([(
        "/slow".to_string(),
        Route {
            delay: Duration::from_millis(800),
            ..article("Slow", &[], "")
        },
    )]));
    let t = start(true);
    std::thread::scope(|scope| {
        let job = scope.spawn(|| t.clip(&s.url("/slow")));
        std::thread::sleep(Duration::from_millis(150));
        let started = std::time::Instant::now();
        t.ok("get_page", json!({"name": "inbox"}));
        assert!(
            started.elapsed() < Duration::from_millis(400),
            "get_page waited for the clip: {:?}",
            started.elapsed()
        );
        assert!(job.join().unwrap().is_ok());
    });
}

#[test]
fn addresses_on_this_computer_are_refused_by_default_and_nothing_is_written() {
    let s = Server::start(HashMap::from([(
        "/a".to_string(),
        article("Secret", &[], ""),
    )]));
    let t = start(false);
    let e = t.clip(&s.url("/a")).unwrap_err();
    assert_eq!(e.code, "invalid", "{e}");
    assert!(e.message.contains("private network"), "{e}");
    assert!(
        !e.message.contains("127.0.0.1"),
        "the message must not carry the address: {e}"
    );
    for bad in [
        "file:///etc/passwd",
        "ftp://example.org/",
        "https://u:p@example.org/",
        "nonsense",
    ] {
        assert_eq!(t.clip(bad).unwrap_err().code, "invalid", "{bad}");
        assert_eq!(
            t.call("clip_existing", json!({"url": bad}))
                .unwrap_err()
                .code,
            "invalid",
            "{bad}"
        );
    }
    assert!(t.web_files().is_empty());
    assert_eq!(t.ok("clip_progress", json!({}))["running"], false);
}

#[test]
fn a_clip_without_an_open_graph_is_refused() {
    let tmp = tempfile::tempdir().unwrap();
    let today = NaiveDate::from_ymd_opt(2026, 10, 3).unwrap();
    let app = AppState::new(AppOptions {
        dirs: Dirs::under(tmp.path()),
        emit: Arc::new(|_, _| {}),
        today: Arc::new(move || today),
        watch: false,
    });
    let e = dispatch(
        &app,
        "clip_page",
        json!({"url": "https://example.org/", "page": "inbox"}),
    )
    .unwrap_err();
    assert_eq!(e.code, "no_graph");
}

#[test]
fn the_sidecar_of_a_clip_gets_the_new_hash_when_the_page_is_clipped_again_and_keeps_its_highlights()
{
    let page = Arc::new(Mutex::new(article("Hash Test", &[], "")));
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let served = page.clone();
    std::thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            let mut buf = [0u8; 2048];
            let _ = stream.read(&mut buf);
            let body = served.lock().unwrap().body.clone();
            let _ = write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = stream.write_all(&body);
        }
    });
    let url = format!("http://127.0.0.1:{}/p", addr.port());
    let t = start(true);
    let first = t.clip(&url).unwrap();
    let path = first["path"].as_str().unwrap().to_string();
    let side = t.ok("annotation_page", json!({"path": path}));
    let sidecar = format!("pages/{}.md", side["page"].as_str().unwrap());
    let before = std::fs::read_to_string(t.graph().join(&sidecar)).unwrap();
    // a highlight, written as the reader would
    std::fs::write(
        t.graph().join(&sidecar),
        format!("{before}- Paragraph 1: the keeper climbed ^a1b2c3\n  hl-cfi:: epubcfi(/6/2!/4/4/1,:0,:20)\n  hl-color:: yellow\n"),
    )
    .unwrap();
    t.ok(
        "open_graph",
        json!({"path": t.graph().to_str().unwrap(), "create": false}),
    );
    let with_highlight = std::fs::read_to_string(t.graph().join(&sidecar)).unwrap();
    *page.lock().unwrap() = article("Hash Test", &[], "A sentence was added.");
    t.clip(&url).unwrap();
    let again = t.ok("annotation_page", json!({"path": path}));
    assert_eq!(again["page"], side["page"]);
    assert_eq!(again["created"], false);
    assert_ne!(
        again["sha"], side["sha"],
        "the book changed, so its hash did"
    );
    let after = std::fs::read_to_string(t.graph().join(&sidecar)).unwrap();
    assert_eq!(
        after,
        with_highlight.replace(
            side["sha"].as_str().unwrap(),
            again["sha"].as_str().unwrap()
        ),
        "only the sha line changed"
    );
    assert!(after.contains("^a1b2c3") && after.contains("hl-cfi:: epubcfi(/6/2!/4/4/1,:0,:20)"));
    // asking again changes nothing
    t.ok("annotation_page", json!({"path": path}));
    assert_eq!(
        std::fs::read_to_string(t.graph().join(&sidecar)).unwrap(),
        after
    );
}

#[test]
fn the_address_of_a_clipped_page_is_not_in_the_log_above_debug() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let _ = log::set_logger(&Listen);
        log::set_max_level(log::LevelFilter::Trace);
    });
    let s = Server::start(HashMap::from([
        (
            "/very-private-reading-marker/post".to_string(),
            article("Marker", &["/very-private-reading-marker/pic.png"], ""),
        ),
        (
            "/very-private-reading-marker/pic.png".to_string(),
            picture(0),
        ),
    ]));
    let t = start(true);
    t.clip(&s.url("/very-private-reading-marker/post")).unwrap();
    // and a failure: the message names the host, not the path
    let e = t
        .clip(&s.url("/very-private-reading-marker/missing"))
        .unwrap_err();
    assert!(!e.message.contains("very-private-reading-marker"), "{e}");
    let lines = LOG.get_or_init(Default::default).lock().unwrap().clone();
    assert!(
        !lines.is_empty(),
        "the logger saw nothing, the test proves nothing"
    );
    // at `debug` the HTTP library names the address it asks for (that is its own log, and `debug` is off unless
    // `RUST_LOG` asks for it); nothing at `info` or above may carry it, and neither may the app's own lines
    for l in &lines {
        let level = l.split(' ').next().unwrap_or("");
        let loud = matches!(level, "INFO" | "WARN" | "ERROR");
        let ours = l.starts_with("DEBUG command ") || l.starts_with("DEBUG clipped");
        if loud || ours {
            assert!(
                !l.contains("very-private-reading-marker"),
                "the address is in the log: {l}"
            );
        }
    }
}
