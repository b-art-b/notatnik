//! Dev server: `POST /api/<command>` (JSON in, JSON out), `GET /poll?after=N` (long-poll for events),
//! `GET /asset?path=assets/x.png`. Exactly the same `dispatch` the Tauri app uses, so UI tests in a
//! normal browser exercise the real index, graph and file writes. Never exposed beyond 127.0.0.1.

mod http;

use http::{Reply, Request};
use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use chrono::NaiveDate;
use clap::Parser;
use notatnik_app::{AppOptions, AppState, dispatch};
use notatnik_graph::Dirs;
use serde_json::{Value, json};

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
    /// Use an in-memory calendar server (calendars Tasks, Birthdays, Personal) instead of connecting to one
    #[arg(long)]
    fake_calendar: bool,
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

fn handle(app: &AppState, events: &Events, req: Request) -> Reply {
    let url = req.url.as_str();
    if req.method == "OPTIONS" {
        return Reply::empty(204);
    }
    if req.method == "GET" && url.starts_with("/poll") {
        let after: u64 = url
            .split_once("after=")
            .and_then(|x| x.1.split('&').next())
            .and_then(|n| n.parse().ok())
            .unwrap_or(0);
        let (last, evs) = events.poll(after, Duration::from_secs(15));
        return Reply::json(200, &json!({"last": last, "events": evs}));
    }
    if req.method == "GET" && url.starts_with("/asset") {
        let path = url.split_once("path=").map(|x| x.1).unwrap_or("");
        let decoded: String = path.replace("%2F", "/").replace("%20", " ");
        return match app.read_asset(&decoded) {
            Ok(bytes) => Reply {
                status: 200,
                content_type: notatnik_app::asset_content_type(&decoded).to_string(),
                body: bytes,
            },
            Err(_) => Reply::empty(404),
        };
    }
    if req.method == "GET" && url == "/health" {
        return Reply::json(200, &json!({"ok": true}));
    }
    if req.method == "POST" {
        if let Some(name) = url.strip_prefix("/api/") {
            let body = String::from_utf8_lossy(&req.body);
            let args: Value = if body.trim().is_empty() {
                json!({})
            } else {
                serde_json::from_str(&body).unwrap_or(Value::Null)
            };
            return match dispatch(app, name, args) {
                Ok(v) => Reply::json(200, &v),
                Err(e) => Reply::json(400, &json!({"code": e.code, "message": e.message})),
            };
        }
    }
    Reply::text(404, "not found")
}

fn main() {
    notatnik_log::init("info");
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
    let mut state = AppState::new(AppOptions {
        dirs: Dirs::under(&device),
        emit,
        today: match today {
            Some(d) => Arc::new(move || d),
            None => Arc::new(|| chrono::Local::now().date_naive()),
        },
        watch: !args.no_watch,
    });
    if args.fake_calendar {
        let cal = notatnik_caldav::fake::FakeServer::new();
        cal.add_calendar("Tasks", &["VTODO"]);
        let birthdays = cal.add_calendar("Birthdays", &["VEVENT"]);
        cal.add_calendar("Personal", &["VEVENT", "VTODO"]);
        // an event every day, so the tests do not depend on the date they run on
        use notatnik_caldav::CalDavClient;
        let _ = cal.put(
            &birthdays,
            "standup.ics",
            "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:standup\r\nSUMMARY:Team standup\r\nDTSTART:20200101T090000\r\nDTEND:20200101T091500\r\nRRULE:FREQ=DAILY\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n",
            &notatnik_caldav::Cond::Create,
        );
        state = state.with_calendar_client(Arc::new(cal));
    }
    let app = Arc::new(state);
    let graph = args.graph.to_string_lossy().into_owned();
    if let Err(e) = dispatch(
        &app,
        "open_graph",
        json!({"path": graph, "create": args.create}),
    ) {
        log::error!("cannot open graph: {e}");
        std::process::exit(1);
    }
    let listener = std::net::TcpListener::bind(("127.0.0.1", args.port)).expect("bind 127.0.0.1");
    let addr = listener
        .local_addr()
        .map_or_else(|_| String::new(), |a| a.to_string());
    // the browser tests wait for this line on stdout
    println!("listening on http://{addr}");
    http::serve(&listener, move |req| handle(&app, &events, req));
}
