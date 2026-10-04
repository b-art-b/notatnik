//! API tests: every command through `dispatch`, against copies of the fixture graphs.
use std::path::Path;
use std::sync::{Arc, Mutex};

use chrono::NaiveDate;
use notatnik_app::{AppOptions, AppState, dispatch};
use notatnik_graph::Dirs;
use serde_json::{Value, json};

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

struct T {
    tmp: tempfile::TempDir,
    app: AppState,
    events: Arc<Mutex<Vec<(String, Value)>>>,
}

fn start_with(fixture: &str, watch: bool, today: &str) -> T {
    let tmp = tempfile::tempdir().unwrap();
    let graph = tmp.path().join("graph");
    copy_dir(
        &Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/graphs")
            .join(fixture),
        &graph,
    );
    let events: Arc<Mutex<Vec<(String, Value)>>> = Arc::default();
    let ev = events.clone();
    let today = NaiveDate::parse_from_str(today, "%Y-%m-%d").unwrap();
    let app = AppState::new(AppOptions {
        dirs: Dirs::under(&tmp.path().join("device")),
        emit: Arc::new(move |n, v| ev.lock().unwrap().push((n.to_string(), v))),
        today: Arc::new(move || today),
        watch,
    });
    let t = T { tmp, app, events };
    t.call(
        "open_graph",
        json!({"path": graph.to_str().unwrap(), "create": false}),
    )
    .unwrap();
    t
}

fn start(fixture: &str) -> T {
    start_with(fixture, false, "2026-10-03")
}

impl T {
    fn call(&self, name: &str, args: Value) -> Result<Value, notatnik_app::ApiError> {
        dispatch(&self.app, name, args)
    }
    fn ok(&self, name: &str, args: Value) -> Value {
        self.call(name, args)
            .unwrap_or_else(|e| panic!("{name}: {e}"))
    }
    fn graph(&self) -> std::path::PathBuf {
        self.tmp.path().join("graph")
    }
    fn read(&self, rel: &str) -> String {
        std::fs::read_to_string(self.graph().join(rel)).unwrap()
    }
}

#[test]
fn page_view_of_the_alpha_page() {
    let t = start("basic");
    let p = t.ok("get_page", json!({"name": "work/projects/alpha"}));
    assert_eq!(p["title"], "Project Alpha");
    assert_eq!(p["path"], "pages/work/projects/alpha.md");
    assert_eq!(p["exists"], true);
    assert_eq!(p["frontmatter"]["tags"], json!(["project", "client-x"]));
    let blocks = p["blocks"].as_array().unwrap();
    assert_eq!(blocks.len(), 4);
    let spec = &blocks[1];
    assert_eq!(spec["task"]["state"], "TODO");
    assert_eq!(spec["task"]["scheduled"], "2026-10-05");
    assert_eq!(spec["task"]["priority"], "high");
    assert_eq!(spec["uid"], "k3x9qa");
    assert_eq!(
        spec["content"][0]["children"][0],
        json!({"type": "text", "text": "Write the spec"})
    );
    assert_eq!(
        spec["text"],
        "TODO Write the spec\nscheduled:: 2026-10-05\npriority:: high"
    );
    let done = &spec["children"][0];
    assert_eq!(done["task"]["state"], "DONE");
    assert_eq!(done["props"][0]["key"], "done");
    assert_eq!(done["props"][0]["hidden"], true);
    // rendered refs
    let goal = &blocks[0]["content"][0]["children"];
    assert!(
        goal.as_array()
            .unwrap()
            .iter()
            .any(|i| i["type"] == "page_ref" && i["target"] == "2026-12-01")
    );
    assert!(
        goal.as_array()
            .unwrap()
            .iter()
            .any(|i| i["type"] == "tag" && i["target"] == "milestone")
    );
    let notes = &blocks[2]["content"][0]["children"];
    assert!(
        notes
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["type"] == "block_ref" && i["uid"] == "k3x9qa")
    );
    assert_eq!(blocks[3]["content"][0]["type"], "query");
    assert_eq!(blocks[3]["content"][0]["text"], "task: todo\nns: work");
    // runtime ids are unique
    let mut ids = Vec::new();
    fn walk(bs: &[Value], ids: &mut Vec<u64>) {
        for b in bs {
            ids.push(b["id"].as_u64().unwrap());
            walk(b["children"].as_array().unwrap(), ids);
        }
    }
    walk(blocks, &mut ids);
    let n = ids.len();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), n);
    assert_eq!(p["child_pages"], json!([]));
}

#[test]
fn missing_pages_and_aliases() {
    let t = start("refs");
    let p = t.ok("get_page", json!({"name": "does/not/exist"}));
    assert_eq!(p["exists"], false);
    assert_eq!(p["hash"], Value::Null);
    assert_eq!(p["path"], "pages/does/not/exist.md");
    assert!(p["blocks"].as_array().unwrap().is_empty());
    let p = t.ok("get_page", json!({"name": "SRC"}));
    assert_eq!(p["name"], "source", "an alias opens the real page");
    assert!(t.call("get_page", json!({"name": "./x"})).is_err());
    let j = t.ok("get_page", json!({"name": "2026-10-03"}));
    assert_eq!(j["is_journal"], true);
    assert_eq!(j["path"], "journals/2026/2026-10-03.md");
}

#[test]
fn edit_undo_redo_roundtrip_is_byte_exact() {
    let t = start("basic");
    let rel = "pages/inbox.md";
    let original = t.read(rel);
    let mut p = t.ok("get_page", json!({"name": "inbox"}));
    let mut hash = p["hash"].clone();
    // 1: edit text of block [0,0]
    let r = t.ok("apply_ops", json!({"page": "inbox", "base_hash": hash, "ops": [{"op": "update_text", "block": {"path": [0, 0]}, "text": "Call the dentist tomorrow #health"}]}));
    p = r["page"].clone();
    hash = p["hash"].clone();
    // 2: indent block [0,1] under [0,0]
    let r = t.ok("apply_ops", json!({"page": "inbox", "base_hash": hash, "ops": [{"op": "indent", "block": {"path": [0, 1]}}]}));
    hash = r["page"]["hash"].clone();
    // 3: delete block [0,1] (the TODO passport, now nested)
    let r = t.ok("apply_ops", json!({"page": "inbox", "base_hash": hash, "ops": [{"op": "delete", "block": {"path": [0, 0, 0]}}]}));
    let after = t.read(rel);
    assert_ne!(after, original);
    assert!(after.contains("tomorrow"));
    let _ = r;
    for _ in 0..3 {
        t.ok("undo", json!({}));
    }
    assert_eq!(
        t.read(rel),
        original,
        "three undos restore the exact original"
    );
    assert!(t.call("undo", json!({})).is_err(), "nothing left to undo");
    for _ in 0..3 {
        t.ok("redo", json!({}));
    }
    assert_eq!(t.read(rel), after, "three redos reapply everything");
}

#[test]
fn undo_is_refused_after_an_external_change() {
    let t = start("basic");
    let p = t.ok("get_page", json!({"name": "inbox"}));
    t.ok("apply_ops", json!({"page": "inbox", "base_hash": p["hash"], "ops": [{"op": "update_text", "block": {"path": [0, 0]}, "text": "changed"}]}));
    std::fs::write(
        t.graph().join("pages/inbox.md"),
        "- written by someone else\n",
    )
    .unwrap();
    let e = t.call("undo", json!({})).unwrap_err();
    assert_eq!(e.code, "undo_refused");
    assert!(e.message.contains("changed on disk"), "{e}");
    assert_eq!(
        t.read("pages/inbox.md"),
        "- written by someone else\n",
        "file untouched"
    );
}

#[test]
fn stale_base_hash_is_a_conflict() {
    let t = start("basic");
    let p = t.ok("get_page", json!({"name": "inbox"}));
    std::fs::write(t.graph().join("pages/inbox.md"), "- other\n").unwrap();
    let e = t.call("apply_ops", json!({"page": "inbox", "base_hash": p["hash"], "ops": [{"op": "delete", "block": {"path": [0]}}]})).unwrap_err();
    assert_eq!(e.code, "conflict");
    assert_eq!(t.read("pages/inbox.md"), "- other\n");
    // stale block hash inside the op
    let p = t.ok("get_page", json!({"name": "inbox"}));
    let e = t
        .call("apply_ops", json!({"page": "inbox", "base_hash": p["hash"], "ops": [{"op": "delete", "block": {"path": [0], "hash": "0000000000000000"}}]}))
        .unwrap_err();
    assert_eq!(e.code, "conflict");
}

#[test]
fn first_content_creates_the_file_and_undo_removes_it() {
    let t = start("basic");
    let rel = "pages/brand/new.md";
    assert!(!t.graph().join(rel).exists());
    let r = t.ok("apply_ops", json!({"page": "brand/new", "base_hash": null, "ops": [{"op": "insert", "parent": [], "index": 0, "text": "first block"}]}));
    assert_eq!(r["page"]["exists"], true);
    assert_eq!(t.read(rel), "- first block\n");
    t.ok("undo", json!({}));
    assert!(
        !t.graph().join(rel).exists(),
        "undoing the creation removes the file"
    );
    assert!(
        !t.graph().join("pages/brand").exists(),
        "and the empty folder"
    );
    t.ok("redo", json!({}));
    assert_eq!(t.read(rel), "- first block\n");
}

#[test]
fn backlinks_search_titles_and_blocks() {
    let t = start("basic");
    let b = t.ok("backlinks", json!({"page": "people/anna"}));
    assert_eq!(b.as_array().unwrap().len(), 3);
    assert_eq!(b[0]["source_page"], "2026-10-01");
    assert_eq!(b[0]["breadcrumbs"], json!(["Started the week"]));
    assert_eq!(b[0]["block"]["children"], json!([]));
    let s = t.ok("search", json!({"text": "zolw", "limit": 5}));
    assert_eq!(s[0]["kind"], "block");
    assert_eq!(
        s[0]["highlights"][0],
        json!([0, 4]),
        "UTF-16 offsets, not bytes: Żółw is 4 code units"
    );
    let titles = t.ok("search_titles", json!({"text": "alp", "limit": 5}));
    assert_eq!(titles[0]["name"], "work/projects/alpha");
    let blk = t.ok("get_block", json!({"uid": "k3x9qa"}));
    assert_eq!(blk["page"], "work/projects/alpha");
    assert_eq!(
        blk["block"]["children"].as_array().unwrap().len(),
        1,
        "with children"
    );
    assert_eq!(t.ok("get_block", json!({"uid": "nope00"})), Value::Null);
}

#[test]
fn namespace_tree_and_children() {
    let t = start("namespaces");
    let top = t.ok("list_namespace", json!({"parent": null}));
    let names: Vec<&str> = top
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        ["100%", "a", "a: b", "people", "work", "workshop", "x"]
    );
    let work = top
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["name"] == "work")
        .unwrap();
    assert_eq!(work["is_virtual"], false);
    assert_eq!(work["has_children"], true);
    let a = top
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["name"] == "a")
        .unwrap();
    assert_eq!(a["is_virtual"], true);
    assert_eq!(a["descendants"], 4);
    assert_eq!(
        t.ok("list_namespace", json!({"parent": "a/b"}))[0]["name"],
        "a/b/c"
    );
    assert_eq!(
        t.ok("list_namespace", json!({"parent": "a/b/c/d"}))[0]["leaf"],
        "deep"
    );
    let p = t.ok("get_page", json!({"name": "work/projects"}));
    assert_eq!(p["exists"], false);
    assert_eq!(
        p["child_pages"],
        json!([
            "work/projects/alpha",
            "work/projects/beta",
            "work/projects/gamma"
        ])
    );
}

#[test]
fn journals_use_the_injected_clock_and_create_nothing() {
    let t = start_with("basic", false, "2026-10-03");
    assert_eq!(t.ok("today", json!({}))["value"], "2026-10-03");
    let days = t.ok("journals", json!({"before": null, "limit": 4}));
    let d: Vec<(&str, bool)> = days
        .as_array()
        .unwrap()
        .iter()
        .map(|x| (x["date"].as_str().unwrap(), x["exists"].as_bool().unwrap()))
        .collect();
    assert_eq!(
        d,
        [
            ("2026-10-03", true),
            ("2026-10-02", true),
            ("2026-10-01", true),
            ("2026-09-30", false)
        ]
    );
    let more = t.ok("journals", json!({"before": "2026-09-30", "limit": 2}));
    assert_eq!(more[0]["date"], "2026-09-29");
    let before: Vec<_> = std::fs::read_dir(t.graph().join("journals"))
        .unwrap()
        .collect();
    let _ = t.ok("get_page", json!({"name": "2026-10-04"}));
    assert_eq!(
        std::fs::read_dir(t.graph().join("journals"))
            .unwrap()
            .count(),
        before.len(),
        "reading an empty day creates no file"
    );
    assert!(!t.graph().join("journals/2026/2026-10-04.md").exists());
}

#[test]
fn rename_plan_and_apply() {
    let t = start("basic");
    let plan = t.ok(
        "rename_plan",
        json!({"old": "people/anna", "new": "team/anna"}),
    );
    assert_eq!(
        plan["file_moves"],
        json!([["pages/people/anna.md", "pages/team/anna.md"]])
    );
    assert_eq!(plan["edits"].as_array().unwrap().len(), 3);
    assert!(t.graph().join("pages/people/anna.md").exists());
    let p = t.ok(
        "rename_apply",
        json!({"old": "people/anna", "new": "team/anna"}),
    );
    assert_eq!(p["name"], "team/anna");
    assert!(
        t.read("pages/work/projects/alpha.md")
            .contains("[[team/anna]]")
    );
    let e = t
        .call("rename_plan", json!({"old": "work", "new": "jobs"}))
        .unwrap_err();
    assert_eq!(e.code, "invalid");
}

#[test]
fn ensure_block_id_edits_only_the_target_line() {
    let t = start("basic");
    let before = t.read("pages/inbox.md");
    let id = t.ok("ensure_block_id", json!({"page": "inbox", "path": [0, 0]}));
    let id = id["value"].as_str().unwrap().to_string();
    assert_eq!(id.len(), 6);
    let after = t.read("pages/inbox.md");
    let diff: Vec<(&str, &str)> = before
        .lines()
        .zip(after.lines())
        .filter(|(a, b)| a != b)
        .collect();
    assert_eq!(diff.len(), 1, "{diff:?}");
    assert_eq!(diff[0].1, format!("{} ^{id}", diff[0].0));
    // asking again returns the same id and changes nothing
    let again = t.ok("ensure_block_id", json!({"page": "inbox", "path": [0, 0]}));
    assert_eq!(again["value"], id);
    assert_eq!(t.read("pages/inbox.md"), after);
    assert!(t.ok("get_block", json!({"uid": id}))["page"] == "inbox");
}

#[test]
fn collapsed_state_is_stored_outside_the_graph_and_survives_restart() {
    let t = start("basic");
    let before = t.read("pages/inbox.md");
    let p = t.ok("get_page", json!({"name": "inbox"}));
    let hash = p["blocks"][0]["hash"].as_str().unwrap().to_string();
    assert_eq!(p["blocks"][0]["collapsed"], false);
    t.ok(
        "set_collapsed",
        json!({"page": "inbox", "collapsed": [hash]}),
    );
    assert_eq!(
        t.ok("get_page", json!({"name": "inbox"}))["blocks"][0]["collapsed"],
        true
    );
    assert_eq!(
        t.read("pages/inbox.md"),
        before,
        "the Markdown file is unchanged"
    );
    // restart: a new AppState with the same device dirs
    let app2 = AppState::new(AppOptions {
        dirs: Dirs::under(&t.tmp.path().join("device")),
        emit: Arc::new(|_, _| {}),
        today: Arc::new(|| NaiveDate::from_ymd_opt(2026, 10, 3).unwrap()),
        watch: false,
    });
    dispatch(
        &app2,
        "open_graph",
        json!({"path": t.graph().to_str().unwrap(), "create": false}),
    )
    .unwrap();
    let p = dispatch(&app2, "get_page", json!({"name": "inbox"})).unwrap();
    assert_eq!(p["blocks"][0]["collapsed"], true);
    assert_eq!(t.read("pages/inbox.md"), before);
}

#[test]
fn runtime_block_ids_survive_edits_and_external_changes() {
    let t = start("basic");
    let p = t.ok("get_page", json!({"name": "inbox"}));
    let id_of = |p: &Value, path: &[usize]| -> u64 {
        let mut b = &p["blocks"][path[0]];
        for i in &path[1..] {
            b = &b["children"][*i];
        }
        b["id"].as_u64().unwrap()
    };
    let (a, b, c) = (id_of(&p, &[0]), id_of(&p, &[0, 1]), id_of(&p, &[0, 2]));
    let r = t.ok("apply_ops", json!({"page": "inbox", "base_hash": p["hash"], "ops": [{"op": "insert", "parent": [], "index": 0, "text": "new on top"}]}));
    let p2 = r["page"].clone();
    assert_eq!(id_of(&p2, &[1]), a);
    assert_eq!(id_of(&p2, &[1, 1]), b);
    assert_eq!(id_of(&p2, &[1, 2]), c);
    // an external rewrite of the whole file keeps ids of unchanged blocks
    std::fs::write(t.graph().join("pages/inbox.md"), "- other first\n- Things to sort out\n  - Call the dentist #health\n  - TODO Renew passport\n    deadline:: 2026-10-20\n  - Read about [[reading/zażółć gęślą jaźń]]\n- new on top\n").unwrap();
    t.app
        .get_page(notatnik_app::dto::GetPageArgs {
            name: "inbox".into(),
        })
        .unwrap();
    let p3 = t.ok("get_page", json!({"name": "inbox"}));
    assert_eq!(id_of(&p3, &[1]), a);
    assert_eq!(id_of(&p3, &[1, 1]), b);
}

#[test]
fn watcher_events_reach_the_emitter() {
    let t = start_with("basic", true, "2026-10-03");
    std::thread::sleep(std::time::Duration::from_millis(300));
    std::fs::write(t.graph().join("pages/inbox.md"), "- changed outside\n").unwrap();
    let start = std::time::Instant::now();
    loop {
        if t.events
            .lock()
            .unwrap()
            .iter()
            .any(|(n, v)| n == "graph-changed" && v["pages"] == json!(["inbox"]))
        {
            break;
        }
        assert!(
            start.elapsed() < std::time::Duration::from_secs(5),
            "no graph-changed event"
        );
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    let p = t.ok("get_page", json!({"name": "inbox"}));
    assert_eq!(p["blocks"][0]["text"], "changed outside");
}

#[test]
fn conflict_files_are_listed_and_readable() {
    let t = start("basic");
    assert_eq!(t.ok("list_conflicts", json!({}))["items"], json!([]));
    std::fs::write(
        t.graph()
            .join("pages/inbox (conflicted copy 2026-10-03 101010).md"),
        "- the other version\n",
    )
    .unwrap();
    let c = t.ok("list_conflicts", json!({}));
    assert_eq!(c["items"].as_array().unwrap().len(), 1);
    let v = t.ok("read_conflict", json!({"path": c["items"][0]}));
    assert_eq!(v["text"], "- the other version\n");
    assert_eq!(v["original"], "inbox");
    assert!(
        v["original_text"]
            .as_str()
            .unwrap()
            .contains("Things to sort out")
    );
    assert!(
        t.call("read_conflict", json!({"path": "pages/inbox.md"}))
            .is_err(),
        "only conflict files"
    );
}

#[test]
fn dispatch_errors() {
    let tmp = tempfile::tempdir().unwrap();
    let app = AppState::new(AppOptions {
        dirs: Dirs::under(tmp.path()),
        emit: Arc::new(|_, _| {}),
        today: Arc::new(|| NaiveDate::from_ymd_opt(2026, 10, 3).unwrap()),
        watch: false,
    });
    assert_eq!(
        dispatch(&app, "get_page", json!({"name": "x"}))
            .unwrap_err()
            .code,
        "no_graph"
    );
    assert_eq!(
        dispatch(&app, "nope", json!({})).unwrap_err().code,
        "invalid"
    );
    assert_eq!(
        dispatch(&app, "get_page", json!({"nam": "x"}))
            .unwrap_err()
            .code,
        "invalid"
    );
    // create a new graph
    let g = tmp.path().join("fresh");
    let info = dispatch(
        &app,
        "open_graph",
        json!({"path": g.to_str().unwrap(), "create": true}),
    )
    .unwrap();
    assert!(info["id"].as_str().unwrap().starts_with('g'));
    assert!(g.join(".notatnik/config.toml").exists());
}

// ---- phase 3 ------------------------------------------------------------------------------------

#[test]
fn agenda_sections_for_the_tasks_fixture() {
    let t = start("tasks");
    let a = t.ok("agenda", json!({"ns": null, "tag": null, "priority": null}));
    let texts = |v: &Value| -> Vec<String> {
        v.as_array()
            .unwrap()
            .iter()
            .map(|i| {
                i["block"]["text"]
                    .as_str()
                    .unwrap()
                    .lines()
                    .next()
                    .unwrap()
                    .to_string()
            })
            .collect()
    };
    assert_eq!(a["today"], "2026-10-03");
    assert_eq!(a["overdue"].as_array().unwrap().len(), 7);
    assert_eq!(
        texts(&a["overdue"])[0],
        "TODO Monthly",
        "oldest first (2026-01-31)"
    );
    assert_eq!(texts(&a["today_items"]), ["TODO Due today"]);
    let days: Vec<&str> = a["upcoming"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["date"].as_str().unwrap())
        .collect();
    assert_eq!(
        days,
        ["2026-10-04", "2026-10-05", "2026-10-07", "2026-10-08"]
    );
    assert_eq!(a["undated"].as_array().unwrap().len(), 8);
    assert!(!serde_json::to_string(&a).unwrap().contains("Far future"));
    // filters
    let w = t.ok(
        "agenda",
        json!({"ns": "work", "tag": null, "priority": null}),
    );
    assert_eq!(w["upcoming"].as_array().unwrap().len(), 1);
    assert_eq!(w["undated"].as_array().unwrap().len(), 1);
    assert_eq!(w["overdue"], json!([]));
    let u = t.ok(
        "agenda",
        json!({"ns": null, "tag": "urgent", "priority": null}),
    );
    assert_eq!(
        u["upcoming"][0]["items"][0]["block"]["text"]
            .as_str()
            .unwrap()
            .lines()
            .next()
            .unwrap(),
        "TODO Work task urgent #urgent"
    );
    let h = t.ok(
        "agenda",
        json!({"ns": null, "tag": null, "priority": "high"}),
    );
    assert_eq!(
        h["upcoming"][0]["items"].as_array().unwrap().len(),
        1,
        "scheduled next week, high"
    );
    assert!(
        t.call(
            "agenda",
            json!({"ns": null, "tag": null, "priority": "urgent"})
        )
        .is_err()
    );
}

#[test]
fn run_query_views_errors_and_saved_queries() {
    let t = start("queries");
    let q = t.ok(
        "run_query",
        json!({"text": "task: todo\nns: work\nview: table", "view": null}),
    );
    assert_eq!(q["view"], "table");
    assert_eq!(q["errors"], json!([]));
    assert_eq!(q["rows"].as_array().unwrap().len(), 5);
    assert_eq!(q["rows"][0]["block"]["task"]["state"], "TODO");
    assert!(
        q["columns"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c == "prop.priority")
    );
    // the view switcher overrides the clause
    let k = t.ok(
        "run_query",
        json!({"text": "task: todo\nview: table", "view": "kanban"}),
    );
    assert_eq!(k["view"], "kanban");
    // grouping
    let g = t.ok(
        "run_query",
        json!({"text": "task: any\ngroup: task", "view": null}),
    );
    assert_eq!(g["group_by"], "task");
    assert_eq!(g["rows"][0]["group"], "doing");
    // page queries
    let p = t.ok(
        "run_query",
        json!({"text": "type: page\ntag: project", "view": null}),
    );
    assert_eq!(p["target"], "page");
    assert_eq!(p["rows"].as_array().unwrap().len(), 2);
    // errors carry the position and a suggestion
    let e = t.ok(
        "run_query",
        json!({"text": "task: todo\ntags: x", "view": null}),
    );
    assert_eq!(e["errors"][0]["line"], 2);
    assert_eq!(e["errors"][0]["suggestion"], "did you mean `tag`?");
    assert_eq!(e["rows"], json!([]));

    // saved queries live in config.toml
    let list = t.ok(
        "save_query",
        json!({"name": "Open work", "query": "task: todo\nns: work"}),
    );
    assert_eq!(list.as_array().unwrap().len(), 1);
    assert!(t.read(".notatnik/config.toml").contains("Open work"));
    assert!(
        t.call("save_query", json!({"name": "bad", "query": "tags: x"}))
            .is_err()
    );
    assert_eq!(
        t.ok("list_saved_queries", json!({}))
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        t.ok("delete_saved_query", json!({"name": "Open work"})),
        json!([])
    );
}

#[test]
fn property_completion_data() {
    let t = start("queries");
    let keys = t.ok("property_keys", json!({}));
    assert!(
        keys["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|k| k == "status")
    );
    let vals = t.ok("property_values", json!({"key": "status"}));
    let v: Vec<&str> = vals["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_str().unwrap())
        .collect();
    assert!(
        v.contains(&"draft") && v.contains(&"reading") && v.contains(&"done"),
        "{v:?}"
    );
}

#[test]
fn completing_a_repeating_task_through_the_api_is_one_undo_step() {
    let t = start("tasks");
    let before = t.read("pages/repeats.md");
    let p = t.ok("get_page", json!({"name": "repeats"}));
    // block [1] is "Weekly" (scheduled 2026-09-28)
    t.ok("apply_ops", json!({"page": "repeats", "base_hash": p["hash"], "ops": [{"op": "set_task_state", "block": {"path": [1]}, "state": "DONE", "done_at": "2026-10-03T08:00"}]}));
    let after = t.read("pages/repeats.md");
    assert!(
        after.contains(
            "- TODO Weekly\n  scheduled:: 2026-10-05\n  repeat:: weekly\n  completed:: 2026-10-03\n"
        ),
        "{after}"
    );
    t.ok("undo", json!({}));
    assert_eq!(t.read("pages/repeats.md"), before);
}

#[test]
fn own_writes_emit_graph_changed_with_the_own_flag() {
    let t = start("basic");
    let p = t.ok("get_page", json!({"name": "inbox"}));
    t.ok("apply_ops", json!({"page": "inbox", "base_hash": p["hash"], "ops": [{"op": "update_text", "block": {"path": [0, 0]}, "text": "x"}]}));
    let ev = t.events.lock().unwrap().clone();
    assert!(
        ev.iter().any(|(n, v)| n == "graph-changed"
            && v["own"] == true
            && v["pages"] == json!(["inbox"])),
        "{ev:?}"
    );
}

#[test]
fn settings_change_only_their_own_file() {
    let t = start("basic");
    let cfg_path = t.graph().join(".notatnik/config.toml");
    let dev_path = t.tmp.path().join("device/config/settings.toml");
    let cfg_before = std::fs::read_to_string(&cfg_path).unwrap();
    let dev_before = std::fs::read_to_string(&dev_path).unwrap();
    // a device setting touches settings.toml only
    let s = t.ok("set_settings", json!({"theme": "dark", "date_locale": "pl-PL", "week_start": null, "completed_log_limit": null}));
    assert_eq!(s["theme"], "dark");
    assert_eq!(std::fs::read_to_string(&cfg_path).unwrap(), cfg_before);
    assert_ne!(std::fs::read_to_string(&dev_path).unwrap(), dev_before);
    let dev_mid = std::fs::read_to_string(&dev_path).unwrap();
    // a graph setting touches config.toml only
    let s = t.ok("set_settings", json!({"theme": null, "date_locale": null, "week_start": "sunday", "completed_log_limit": 3}));
    assert_eq!(
        (s["week_start"].as_str(), s["completed_log_limit"].as_u64()),
        (Some("sunday"), Some(3))
    );
    assert_eq!(std::fs::read_to_string(&dev_path).unwrap(), dev_mid);
    let cfg_after = std::fs::read_to_string(&cfg_path).unwrap();
    assert!(
        cfg_after.contains("sunday") && cfg_after.contains("completed_log_limit = 3"),
        "{cfg_after}"
    );
    assert_eq!(t.ok("graph_info", json!({}))["week_start"], "sunday");
    assert!(t.call("set_settings", json!({"theme": "neon", "date_locale": null, "week_start": null, "completed_log_limit": null})).is_err());
}
