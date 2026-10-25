//! Two graphs in one app (MG-1) and moving things between them (MG-2 to MG-4), through `dispatch_slot`.
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use chrono::NaiveDate;
use notatnik_app::{ApiError, AppOptions, AppState, dispatch_slot};
use notatnik_graph::Dirs;
use serde_json::{Value, json};

struct W {
    tmp: tempfile::TempDir,
    app: AppState,
    events: Arc<Mutex<Vec<(String, Value)>>>,
}

fn w() -> W {
    let tmp = tempfile::tempdir().unwrap();
    let events: Arc<Mutex<Vec<(String, Value)>>> = Arc::default();
    let ev = events.clone();
    let today = NaiveDate::parse_from_str("2026-10-03", "%Y-%m-%d").unwrap();
    let app = AppState::new(AppOptions {
        dirs: Dirs::under(&tmp.path().join("device")),
        emit: Arc::new(move |n, v| ev.lock().unwrap().push((n.to_string(), v))),
        today: Arc::new(move || today),
        watch: false,
    });
    W { tmp, app, events }
}

impl W {
    fn call(&self, slot: &str, name: &str, args: Value) -> Result<Value, ApiError> {
        dispatch_slot(&self.app, slot, name, args)
    }
    fn ok(&self, slot: &str, name: &str, args: Value) -> Value {
        self.call(slot, name, args)
            .unwrap_or_else(|e| panic!("{slot}/{name}: {e}"))
    }
    /// Makes a graph folder `name` and opens it in `slot`.
    fn open(&self, slot: &str, name: &str) -> PathBuf {
        let root = self.tmp.path().join(name);
        self.ok(
            slot,
            "open_graph",
            json!({"path": root.to_str().unwrap(), "create": true}),
        );
        root
    }
    /// Writes a page through the API of `slot` (an empty page gets a first block).
    fn write(&self, slot: &str, page: &str, text: &str) {
        self.ok(
            slot,
            "apply_ops",
            json!({"page": page, "base_hash": null, "ops": [
                {"op": "insert", "parent": [], "index": 0, "text": text}
            ]}),
        );
    }
    fn names(&self, slot: &str) -> Vec<String> {
        let rows = self.ok(slot, "all_pages", json!({}));
        rows.as_array()
            .unwrap()
            .iter()
            .map(|r| r["name"].as_str().unwrap().to_string())
            .collect()
    }
}

fn read(root: &Path, rel: &str) -> String {
    std::fs::read_to_string(root.join(rel)).unwrap()
}

#[test]
fn two_graphs_are_open_side_by_side_with_nothing_shared() {
    let w = w();
    let a = w.open("", "alpha-graph");
    let b = w.open("1", "beta-graph");
    w.write("", "only in a", "from a");
    w.write("1", "only in b", "from b");

    assert!(w.names("").contains(&"only in a".to_string()));
    assert!(!w.names("").contains(&"only in b".to_string()));
    assert!(w.names("1").contains(&"only in b".to_string()));
    assert!(!w.names("1").contains(&"only in a".to_string()));
    assert!(a.join("pages/only in a.md").exists());
    assert!(b.join("pages/only in b.md").exists());

    // the graph info of each slot is its own
    let ia = w.ok("", "graph_info", json!({}));
    let ib = w.ok("1", "graph_info", json!({}));
    assert_ne!(ia["id"], ib["id"]);

    // undo is separate: undoing in slot 1 does not touch slot 0
    w.ok("1", "undo", json!({}));
    assert!(a.join("pages/only in a.md").exists());
    assert!(!b.join("pages/only in b.md").exists());

    // the list says what is open where
    let slots = w.ok("", "slots", json!({}));
    let slots = slots.as_array().unwrap();
    assert_eq!(slots.len(), 2);
    assert_eq!(slots[0]["slot"], "");
    assert_eq!(slots[0]["name"], "alpha-graph");
    assert_eq!(slots[1]["slot"], "1");
    assert_eq!(slots[1]["name"], "beta-graph");
}

#[test]
fn a_graph_is_not_opened_twice_and_the_first_slot_stays_the_start_graph() {
    let w = w();
    let a = w.open("", "alpha-graph");
    let err = w
        .call(
            "1",
            "open_graph",
            json!({"path": a.to_str().unwrap(), "create": false}),
        )
        .unwrap_err();
    assert_eq!(err.code, "already_open");
    // nor the other way round
    let b = w.open("1", "beta-graph");
    let err = w
        .call(
            "",
            "open_graph",
            json!({"path": b.to_str().unwrap(), "create": false}),
        )
        .unwrap_err();
    assert_eq!(err.code, "already_open");
    // opening the same graph again in its own slot is the normal "reload"
    w.ok(
        "",
        "open_graph",
        json!({"path": a.to_str().unwrap(), "create": false}),
    );

    // the app starts with the first graph, not with the one opened beside it
    let dev =
        notatnik_graph::DeviceSettings::load(&Dirs::under(&w.tmp.path().join("device"))).unwrap();
    let ia = w.ok("", "graph_info", json!({}));
    assert_eq!(dev.last_graph.as_deref(), ia["id"].as_str());
    assert_eq!(dev.graphs.len(), 2, "both are in the history");
}

#[test]
fn closing_a_slot_frees_its_graph_and_leaves_the_other_open() {
    let w = w();
    w.open("", "alpha-graph");
    let b = w.open("1", "beta-graph");
    w.write("1", "page b", "x");

    let left = w.ok("", "close_slot", json!({"slot": "1"}));
    assert_eq!(left.as_array().unwrap().len(), 1);
    assert_eq!(
        w.call("1", "all_pages", json!({})).unwrap_err().code,
        "no_graph"
    );
    assert!(w.call("", "all_pages", json!({})).is_ok());
    assert_eq!(
        w.call("", "close_slot", json!({"slot": ""}))
            .unwrap_err()
            .code,
        "invalid"
    );
    // its folder can be opened in the first slot now
    w.ok(
        "",
        "open_graph",
        json!({"path": b.to_str().unwrap(), "create": false}),
    );
    assert!(w.names("").contains(&"page b".to_string()));
}

#[test]
fn events_of_an_extra_slot_say_which_slot_they_come_from() {
    let w = w();
    w.open("", "alpha-graph");
    w.open("1", "beta-graph");
    w.events.lock().unwrap().clear();
    w.write("", "page a", "x");
    w.write("1", "page b", "y");
    let events = w.events.lock().unwrap().clone();
    let of = |page: &str| {
        events
            .iter()
            .find(|(n, v)| n == "graph-changed" && v["pages"].to_string().contains(page))
            .map(|(_, v)| v.clone())
            .unwrap_or_else(|| panic!("no graph-changed for {page}: {events:?}"))
    };
    assert!(
        of("page a").get("slot").is_none(),
        "the first graph's events have no slot"
    );
    assert_eq!(of("page b")["slot"], "1");
}

#[test]
fn a_slot_name_is_short_and_plain() {
    let w = w();
    w.open("", "alpha-graph");
    for bad in ["../x", "a b", "waytoolongslot", "é"] {
        let e = w
            .call(bad, "open_graph", json!({"path": "/tmp/x", "create": true}))
            .unwrap_err();
        assert_eq!(e.code, "invalid", "{bad}");
    }
}
