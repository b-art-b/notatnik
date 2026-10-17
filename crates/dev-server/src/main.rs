//! Dev server: `POST /api/<command>` (JSON in, JSON out), `GET /poll?after=N` (long-poll for events),
//! `GET /asset?path=assets/x.png`. Exactly the same `dispatch` the Tauri app uses, so UI tests in a
//! normal browser exercise the real index, graph and file writes. Never exposed beyond 127.0.0.1.

use std::io::Read;
use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use chrono::NaiveDate;
use clap::Parser;
use notatnik_app::{AppOptions, AppState, dispatch};
use notatnik_graph::Dirs;
use serde_json::{Value, json};
use tiny_http::{Header, Method, Request, Response, Server, StatusCode};

#[derive(Parser)]
#[command(about = "HTTP dev backend for the notatnik UI (127.0.0.1 only)")]
struct Args {
    /// Graph folder to open at start
    #[arg(long)]
    graph: PathBuf,
    /// Create the graph if the folder has none
    #[arg(long)]
    create: bool,
    /// Port (0 = pick a free one; the chosen address is printed)
    #[arg(long, default_value_t = 0)]
    port: u16,
    /// Treat this date as today (YYYY-MM-DD)
    #[arg(long)]
    today: Option<String>,
    /// Folder for the device-local cache and settings (default: a fresh temp folder)
    #[arg(long)]
    device_dir: Option<PathBuf>,
    /// Don't watch the graph folder for external changes
    #[arg(long)]
    no_watch: bool,
}

/// Recent events for long-polling clients: `(id, name, payload)`.
/// `(last id, recent events)`
type EventLog = (u64, Vec<(u64, String, Value)>);

#[derive(Default)]
struct Events {
    log: Mutex<EventLog>,
    changed: Condvar,
}

impl Events {
    fn push(&self, name: &str, payload: Value) {
        let mut g = self
            .log
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        g.0 += 1;
        let id = g.0;
        g.1.push((id, name.to_string(), payload));
        if g.1.len() > 500 {
            g.1.remove(0);
        }
        self.changed.notify_all();
    }

    /// Events after `after`; waits up to `wait` for the first one.
    fn poll(&self, after: u64, wait: Duration) -> (u64, Vec<Value>) {
        let mut g = self
            .log
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if g.1.iter().all(|e| e.0 <= after) {
            g = self
                .changed
                .wait_timeout(g, wait)
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .0;
        }
        let evs: Vec<Value> =
            g.1.iter()
                .filter(|e| e.0 > after)
                .map(|(id, n, p)| json!({"id": id, "name": n, "payload": p}))
                .collect();
        (g.0, evs)
    }
}

fn header(k: &str, v: &str) -> Header {
    Header::from_bytes(k.as_bytes(), v.as_bytes()).expect("static header")
}

fn cors<R: Read>(r: Response<R>) -> Response<R> {
    r.with_header(header("Access-Control-Allow-Origin", "*"))
        .with_header(header("Access-Control-Allow-Headers", "content-type"))
        .with_header(header("Access-Control-Allow-Methods", "GET, POST, OPTIONS"))
}

fn json_response(status: u16, v: &Value) -> Response<std::io::Cursor<Vec<u8>>> {
    cors(
        Response::from_string(v.to_string())
            .with_status_code(StatusCode(status))
            .with_header(header("Content-Type", "application/json")),
    )
}

fn handle(app: &AppState, events: &Events, mut req: Request) {
    let url = req.url().to_string();
    let method = req.method().clone();
    if method == Method::Options {
        let _ = req.respond(cors(Response::empty(204)));
        return;
    }
    if method == Method::Get && url.starts_with("/poll") {
        let after: u64 = url
            .split_once("after=")
            .and_then(|x| x.1.split('&').next())
            .and_then(|n| n.parse().ok())
            .unwrap_or(0);
        let (last, evs) = events.poll(after, Duration::from_secs(15));
        let _ = req.respond(json_response(200, &json!({"last": last, "events": evs})));
        return;
    }
    if method == Method::Get && url.starts_with("/asset") {
        let path = url.split_once("path=").map(|x| x.1).unwrap_or("");
        let decoded: String = path.replace("%2F", "/").replace("%20", " ");
        let resp = match app.read_asset(&decoded) {
            Ok(bytes) => cors(Response::from_data(bytes).with_header(header(
                "Content-Type",
                notatnik_app::asset_content_type(&decoded),
            ))),
            Err(_) => cors(Response::from_data(Vec::new()).with_status_code(StatusCode(404))),
        };
        let _ = req.respond(resp);
        return;
    }
    if method == Method::Get && url == "/health" {
        let _ = req.respond(json_response(200, &json!({"ok": true})));
        return;
    }
    if method == Method::Post {
        if let Some(name) = url.strip_prefix("/api/") {
            let mut body = String::new();
            let _ = req.as_reader().read_to_string(&mut body);
            let args: Value = if body.trim().is_empty() {
                json!({})
            } else {
                serde_json::from_str(&body).unwrap_or(Value::Null)
            };
            let resp = match dispatch(app, name, args) {
                Ok(v) => json_response(200, &v),
                Err(e) => json_response(400, &json!({"code": e.code, "message": e.message})),
            };
            let _ = req.respond(resp);
            return;
        }
    }
    let _ = req.respond(cors(
        Response::from_string("not found").with_status_code(StatusCode(404)),
    ));
}

fn main() {
    let args = Args::parse();
    let events: Arc<Events> = Arc::default();
    let device = args.device_dir.clone().unwrap_or_else(|| {
        std::env::temp_dir().join(format!("notatnik-dev-{}", std::process::id()))
    });
    let today = args
        .today
        .as_deref()
        .map(|t| NaiveDate::parse_from_str(t, "%Y-%m-%d").expect("--today must be YYYY-MM-DD"));
    let ev = events.clone();
    let emit: notatnik_app::Emitter = Arc::new(move |name, payload| ev.push(name, payload));
    let app = Arc::new(AppState::new(AppOptions {
        dirs: Dirs::under(&device),
        emit,
        today: match today {
            Some(d) => Arc::new(move || d),
            None => Arc::new(|| chrono::Local::now().date_naive()),
        },
        watch: !args.no_watch,
    }));
    let graph = args.graph.to_string_lossy().into_owned();
    if let Err(e) = dispatch(
        &app,
        "open_graph",
        json!({"path": graph, "create": args.create}),
    ) {
        eprintln!("cannot open graph: {e}");
        std::process::exit(1);
    }
    let server = Server::http(("127.0.0.1", args.port)).expect("bind 127.0.0.1");
    let addr = server
        .server_addr()
        .to_ip()
        .map_or_else(String::new, |a| a.to_string());
    println!("listening on http://{addr}");
    for req in server.incoming_requests() {
        let app = app.clone();
        let events = events.clone();
        std::thread::spawn(move || handle(&app, &events, req));
    }
}
