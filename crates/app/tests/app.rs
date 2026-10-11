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
    assert_eq!(
        spec["task"]["priority"], "A",
        "`high` in the file is the first label"
    );
    assert_eq!(spec["task"]["priority_rank"], 0);
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
    // a whole namespace moves with its pages (NS-8)
    let plan = t.ok("rename_plan", json!({"old": "work", "new": "jobs"}));
    assert!(plan["file_moves"].as_array().unwrap().len() >= 2, "{plan}");
    // moving a namespace into itself is refused
    let e = t
        .call(
            "rename_plan",
            json!({"old": "work", "new": "work/projects/x"}),
        )
        .unwrap_err();
    assert_eq!(e.code, "invalid");
    assert!(e.message.contains("into itself"), "{}", e.message);
    // nothing to revert when no rename stopped half way
    assert!(t.call("rename_revert", json!({})).is_err());
    let p = t.ok("rename_apply", json!({"old": "work", "new": "jobs"}));
    assert_eq!(p["name"], "jobs");
    assert!(t.graph().join("pages/jobs/projects/alpha.md").exists());
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
    // beyond the 7-day window: in `later`, nowhere else
    assert_eq!(a["later"].as_array().unwrap().len(), 1);
    assert!(texts(&a["later"])[0].contains("Far future"));
    let rest = json!([a["overdue"], a["today_items"], a["upcoming"], a["undated"]]);
    assert!(!rest.to_string().contains("Far future"));
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
    let h = t.ok("agenda", json!({"ns": null, "tag": null, "priority": "A"}));
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

/// QV-12: `[[]]` in a query is the page the query sits on.
#[test]
fn empty_link_in_a_query_means_the_page_it_sits_on() {
    let t = start("queries");
    let named = t.ok(
        "run_query",
        json!({"text": "page: [[work/tasks]]\ntask: todo", "view": null}),
    );
    let here = t.ok(
        "run_query",
        json!({"text": "page: [[]]\ntask: todo", "view": null, "page": "work/tasks"}),
    );
    assert_eq!(here["errors"], json!([]));
    assert!(!here["rows"].as_array().unwrap().is_empty());
    assert_eq!(here["rows"], named["rows"]);
    // without a page (agenda, sidebar) it stays an error instead of matching everything
    let none = t.ok(
        "run_query",
        json!({"text": "page: [[]]\ntask: todo", "view": null}),
    );
    assert!(!none["errors"].as_array().unwrap().is_empty());
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
    // the link-bracket flag is a device setting too
    let s = t.ok("set_settings", json!({"link_brackets": true}));
    assert_eq!(s["link_brackets"], true);
    assert_eq!(std::fs::read_to_string(&cfg_path).unwrap(), cfg_after);
    assert!(
        std::fs::read_to_string(&dev_path)
            .unwrap()
            .contains("link_brackets = true")
    );
    // date format and language
    let s = t.ok(
        "set_settings",
        json!({"date_format": "iso", "date_locale": "de-DE"}),
    );
    assert_eq!(
        (s["date_format"].as_str(), s["date_locale"].as_str()),
        (Some("iso"), Some("de-DE"))
    );
    let s = t.ok(
        "set_settings",
        json!({"date_format": "custom", "date_pattern": "dddd, D MMMM YYYY"}),
    );
    assert_eq!(
        (s["date_format"].as_str(), s["date_pattern"].as_str()),
        (Some("custom"), Some("dddd, D MMMM YYYY"))
    );
    assert!(
        std::fs::read_to_string(&dev_path)
            .unwrap()
            .contains("date_pattern = \"dddd, D MMMM YYYY\"")
    );
    assert!(
        t.call("set_settings", json!({"date_pattern": "a\nb"}))
            .is_err()
    );
    assert!(
        t.call("set_settings", json!({"date_format": "weird"}))
            .is_err()
    );
    assert!(
        t.call("set_settings", json!({"date_locale": "x y"}))
            .is_err()
    );
    assert!(t.call("set_settings", json!({"theme": "neon", "date_locale": null, "week_start": null, "completed_log_limit": null})).is_err());
}

/// Task logbook: the drawer is hidden from the rendered text, listed as entries, and kept in the raw text.
#[test]
fn logbook_is_listed_and_not_rendered() {
    let t = start("edge-cases");
    let p = t.ok("get_page", json!({"name": "logbook"}));
    let first = &p["blocks"][0];
    let log = first["logbook"].as_array().unwrap();
    assert_eq!(log.len(), 2, "{first}");
    assert_eq!(log[0]["when"], "2026-10-04 14:30");
    assert_eq!(log[0]["text"], "Outline done");
    assert_eq!(log[1]["when"], "2026-10-05");
    // the note is rendered: `[[Anna]]` is a page link, not text
    let rendered_note = log[1]["content"].to_string();
    assert!(
        rendered_note.contains("page_ref") && rendered_note.contains("Anna"),
        "{rendered_note}"
    );
    // rendered content has the title only; the drawer lines are not text
    let rendered = first["content"].to_string();
    assert!(rendered.contains("Write the spec"), "{rendered}");
    assert!(
        !rendered.contains("LOGBOOK") && !rendered.contains("Outline done"),
        "{rendered}"
    );
    // the raw editor text keeps the drawer
    let raw = first["text"].as_str().unwrap();
    assert!(raw.contains(":LOGBOOK:") && raw.contains(":END:"), "{raw}");
    // the property above the drawer is still a property, and the child is still a child
    assert_eq!(first["props"][0]["key"], "deadline");
    assert_eq!(first["children"].as_array().unwrap().len(), 1);
    // without `:END:` it is plain text and no logbook
    let second = &p["blocks"][1];
    assert!(second["logbook"].as_array().unwrap().is_empty());
    assert!(second["content"].to_string().contains("LOGBOOK"));
    // the entry text is searchable and a no-op rewrite keeps the file identical
    let src = std::fs::read_to_string(t.graph().join("pages/logbook.md")).unwrap();
    let r = t.ok(
        "apply_ops",
        json!({"page": "logbook", "base_hash": p["hash"], "ops": []}),
    );
    assert_eq!(
        std::fs::read_to_string(t.graph().join("pages/logbook.md")).unwrap(),
        src
    );
    let _ = r;
}

/// "Hierarchy": all pages below a page, at every depth, in tree order, virtual ones included.
#[test]
fn subpages_lists_the_whole_subtree() {
    let t = start("namespaces");
    let r = t.ok("subpages", json!({"page": "work"}));
    let names: Vec<&str> = r
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["name"].as_str().unwrap())
        .collect();
    // `gamma` is only a link target (from people/anna): it is listed too
    assert_eq!(
        names,
        [
            "work/projects",
            "work/projects/alpha",
            "work/projects/beta",
            "work/projects/gamma"
        ]
    );
    assert_eq!(r[0]["is_virtual"], true); // `work/projects` only exists because of its children
    // a prefix is not a namespace: `workshop` is not below `work`, and a leaf has nothing below it
    assert!(!names.iter().any(|n| n.starts_with("workshop")));
    assert_eq!(
        t.ok("subpages", json!({"page": "work/projects/beta"})),
        json!([])
    );
    assert_eq!(
        t.ok("subpages", json!({"page": "a"}))
            .as_array()
            .unwrap()
            .len(),
        4
    );
}

/// The all-pages list: files and virtual pages, journals flagged, counts and modification time.
#[test]
fn all_pages_lists_everything_with_counts() {
    let t = start("basic");
    let rows = t.ok("all_pages", json!({}));
    let rows = rows.as_array().unwrap();
    assert!(!rows.is_empty());
    let alpha = rows.iter().find(|r| r["name"] == "work/projects/alpha");
    let alpha = alpha.expect("alpha is listed");
    assert_eq!(alpha["is_virtual"], false);
    assert!(alpha["blocks"].as_u64().unwrap() > 0);
    assert!(alpha["modified"].as_str().unwrap().len() == 16);
    // a page that is only referenced is virtual, has no modification time but has backlinks
    let virt = rows
        .iter()
        .find(|r| r["is_virtual"] == true)
        .expect("a virtual page");
    assert!(virt["modified"].is_null() && virt["blocks"] == 0);
    // journals are flagged
    assert!(rows.iter().any(|r| r["is_journal"] == true));
    // sorted by name, case-insensitively
    let names: Vec<String> = rows
        .iter()
        .map(|r| r["name"].as_str().unwrap().to_lowercase())
        .collect();
    let mut sorted = names.clone();
    sorted.sort();
    assert_eq!(names, sorted);
}

/// LN-4: plain-text mentions are listed (whole word, any case), not linked ones, and "link" text is exact.
#[test]
fn unlinked_references_list_whole_word_mentions() {
    let t = start("basic");
    let pages = t.graph().join("pages");
    std::fs::write(pages.join("Anna.md"), "- Anna is a person\n").unwrap();
    std::fs::write(
        pages.join("notes.md"),
        "- met anna today\n- Anna's idea\n- Annabelle came\n- linked [[Anna]] and Anna again\n- code `Anna` and #Anna\n  - nested mention of Anna here\n",
    )
    .unwrap();
    for p in ["Anna", "notes"] {
        t.ok("get_page", json!({"name": p}));
    }
    let r = t.ok("unlinked_refs", json!({"page": "Anna"}));
    // (the basic graph's own `people/anna` mentions "Anna Kowalska" too; this test looks at `notes`)
    let r: Vec<Value> = r
        .as_array()
        .unwrap()
        .iter()
        .filter(|u| u["source_page"] == "notes")
        .cloned()
        .collect();
    let texts: Vec<&str> = r
        .iter()
        .map(|u| u["block"]["text"].as_str().unwrap())
        .collect();
    assert_eq!(
        texts,
        [
            "met anna today",
            "Anna's idea",
            "nested mention of Anna here"
        ],
        "{r:?}"
    );
    assert_eq!(r[0]["link_text"], "met [[Anna|anna]] today");
    assert_eq!(r[1]["link_text"], "[[Anna]]'s idea");
    assert_eq!(r[2]["breadcrumbs"][0], "code `Anna` and #Anna");
    // the page's own blocks are never listed
    let all = t.ok("unlinked_refs", json!({"page": "Anna"}));
    assert!(
        all.as_array()
            .unwrap()
            .iter()
            .all(|u| u["source_page"] != "Anna")
    );
    // applying the link text changes only that block, and the mention is then a linked reference
    let p = t.ok("get_page", json!({"name": "notes"}));
    let path = r[0]["block_path"].clone();
    t.ok(
        "apply_ops",
        json!({"page": "notes", "base_hash": p["hash"], "ops": [
            {"op": "update_text", "block": {"path": path, "hash": null}, "text": r[0]["link_text"]}
        ]}),
    );
    let src = std::fs::read_to_string(pages.join("notes.md")).unwrap();
    assert!(
        src.starts_with("- met [[Anna|anna]] today\n- Anna's idea\n"),
        "{src}"
    );
    let again = t.ok("unlinked_refs", json!({"page": "Anna"}));
    let again = again.as_array().unwrap();
    assert_eq!(
        again.iter().filter(|u| u["source_page"] == "notes").count(),
        2
    );
    let back = t.ok("backlinks", json!({"page": "Anna"}));
    assert!(back.as_array().unwrap().len() >= 2);
}

/// Pages grouped by a page property: values are grouped case-insensitively, lists put a page in several groups,
/// pages without the property come last, and the keys in use are offered for the picker.
#[test]
fn pages_are_grouped_by_a_page_property() {
    let t = start("basic");
    let pages = t.graph().join("pages");
    std::fs::write(pages.join("Ann.md"), "---\ntype: person\n---\n- a\n").unwrap();
    std::fs::write(pages.join("bob.md"), "---\ntype: Person\n---\n- b\n").unwrap();
    std::fs::write(
        pages.join("Acme.md"),
        "---\ntype: client\nteams: [red, blue]\n---\n- c\n",
    )
    .unwrap();
    std::fs::write(pages.join("Zed.md"), "---\nteams: [blue]\n---\n- z\n").unwrap();
    for p in ["Ann", "bob", "Acme", "Zed"] {
        t.ok("get_page", json!({"name": p}));
    }
    let g = t.ok("page_groups", json!({"key": "type"}));
    let g = g.as_array().unwrap();
    let find = |v: Option<&str>| {
        g.iter()
            .find(|x| x["value"].as_str() == v)
            .unwrap_or_else(|| panic!("group {v:?} in {g:?}"))["pages"]
            .clone()
    };
    assert_eq!(find(Some("client")), json!(["Acme"]));
    // `person` and `Person` are one group, named as first written
    let person = g.iter().find(|x| {
        x["value"]
            .as_str()
            .is_some_and(|v| v.eq_ignore_ascii_case("person"))
    });
    assert_eq!(person.unwrap()["pages"], json!(["Ann", "bob"]));
    // the pages without the property are the last group and don't include the ones that have it
    let last = g.last().unwrap();
    assert!(last["value"].is_null());
    let none: Vec<&str> = last["pages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p.as_str().unwrap())
        .collect();
    assert!(none.contains(&"Zed") && none.contains(&"inbox"), "{none:?}");
    assert!(!none.contains(&"Ann") && !none.contains(&"Acme"));
    // a list: the page is in each group
    let teams = t.ok("page_groups", json!({"key": "teams"}));
    let teams = teams.as_array().unwrap();
    let blue = teams.iter().find(|x| x["value"] == "blue").unwrap();
    assert_eq!(blue["pages"], json!(["Acme", "Zed"]));
    let red = teams.iter().find(|x| x["value"] == "red").unwrap();
    assert_eq!(red["pages"], json!(["Acme"]));
    // journals never appear
    assert!(!none.iter().any(|n| n.starts_with("2026-")));
    // the keys in use, most used first
    let keys = t.ok("page_property_keys", json!({}));
    let keys: Vec<(&str, u64)> = keys
        .as_array()
        .unwrap()
        .iter()
        .map(|k| (k["key"].as_str().unwrap(), k["pages"].as_u64().unwrap()))
        .collect();
    assert!(
        keys.contains(&("type", 3)) && keys.contains(&("teams", 2)),
        "{keys:?}"
    );
}

/// Page icons: rules in the graph's config (`type: person` → 👤), a page's own `icon:` wins, rules are validated and
/// stored in the graph's config.toml only.
#[test]
fn page_icons_come_from_rules_and_from_the_page() {
    let t = start("basic");
    let pages = t.graph().join("pages");
    std::fs::write(pages.join("Ann.md"), "---\ntype: person\n---\n- a\n").unwrap();
    std::fs::write(
        pages.join("Bob.md"),
        "---\ntype: Person\nicon: 🧑‍🔧\n---\n- b\n",
    )
    .unwrap();
    std::fs::write(pages.join("Acme.md"), "---\ntype: client\n---\n- c\n").unwrap();
    for p in ["Ann", "Bob", "Acme"] {
        t.ok("get_page", json!({"name": p}));
    }
    // before any rule only Bob's own `icon:` counts
    let first = t.ok("page_icons", json!({}));
    assert_eq!(first.as_array().unwrap().len(), 1);
    assert_eq!(first[0]["name"], "Bob");
    let dev_path = t.tmp.path().join("device/config/settings.toml");
    let dev_before = std::fs::read_to_string(&dev_path).unwrap();
    let s = t.ok(
        "set_settings",
        json!({"page_icons": [
            {"key": "type", "value": "person", "icon": "👤"},
            {"key": "type", "value": "client", "icon": "🏢"}
        ]}),
    );
    assert_eq!(s["page_icons"][1]["icon"], "🏢");
    // graph config only
    assert_eq!(std::fs::read_to_string(&dev_path).unwrap(), dev_before);
    let cfg = std::fs::read_to_string(t.graph().join(".notatnik/config.toml")).unwrap();
    assert!(
        cfg.contains("[[page_icons]]") && cfg.contains("icon = \"👤\""),
        "{cfg}"
    );
    let icons = t.ok("page_icons", json!({}));
    let get = |n: &str| {
        icons
            .as_array()
            .unwrap()
            .iter()
            .find(|i| i["name"] == n)
            .map(|i| i["icon"].as_str().unwrap().to_string())
    };
    assert_eq!(get("Ann").as_deref(), Some("👤"));
    assert_eq!(get("Acme").as_deref(), Some("🏢"));
    assert_eq!(
        get("Bob").as_deref(),
        Some("🧑‍🔧"),
        "the page's own icon wins over `type: Person`"
    );
    assert_eq!(get("inbox"), None);
    // invalid rules are refused and change nothing
    for bad in [
        json!([{"key": "", "value": "x", "icon": "👤"}]),
        json!([{"key": "type", "value": "x", "icon": ""}]),
        json!([{"key": "type", "value": "x", "icon": "this is far too long"}]),
    ] {
        assert!(t.call("set_settings", json!({"page_icons": bad})).is_err());
    }
    assert_eq!(
        t.ok("get_settings", json!({}))["page_icons"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    // removing every rule leaves only the explicit icons
    t.ok("set_settings", json!({"page_icons": []}));
    let icons = t.ok("page_icons", json!({}));
    assert_eq!(icons.as_array().unwrap().len(), 1);
}

/// A name used as a path segment: `Renewal` in `P/Capgemini/Renewal/Siemens Energy`, `P/EY/Renewal/APotheke` and
/// `X/Renewal/Snowflake`. The `path:` query clause and the segment view find every place.
#[test]
fn a_path_segment_is_found_in_every_place_it_occurs() {
    let t = start("basic");
    let pages = t.graph().join("pages");
    for (p, body) in [
        (
            "P/Capgemini/Renewal/Siemens Energy.md",
            "- TODO call Siemens\n",
        ),
        (
            "P/EY/Renewal/APotheke.md",
            "- TODO send offer\n- DONE kickoff\n",
        ),
        ("X/Renewal/Snowflake.md", "- TODO renew licence\n"),
        ("P/EY/Other.md", "- TODO not a renewal\n"),
        ("Renewals.md", "- TODO a prefix is not a segment\n"),
        ("renewal.md", "- the top-level page with the same name\n"),
    ] {
        std::fs::create_dir_all(pages.join(p).parent().unwrap()).unwrap();
        std::fs::write(pages.join(p), body).unwrap();
        t.ok("get_page", json!({"name": p.trim_end_matches(".md")}));
    }
    let v = t.ok("segment_view", json!({"segment": "renewal"}));
    let groups = v["groups"].as_array().unwrap();
    let occ: Vec<&str> = groups
        .iter()
        .map(|g| g["occurrence"].as_str().unwrap())
        .collect();
    // every place, in path order; the top-level `renewal` page is one too (case-insensitive); `Renewals` is not
    assert_eq!(
        occ,
        [
            "P/Capgemini/Renewal",
            "P/EY/Renewal",
            "renewal",
            "X/Renewal"
        ]
    );
    let below = |i: usize| -> Vec<&str> {
        groups[i]["pages"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p["name"].as_str().unwrap())
            .collect()
    };
    assert_eq!(below(0), ["P/Capgemini/Renewal/Siemens Energy"]);
    assert_eq!(below(1), ["P/EY/Renewal/APotheke"]);
    assert_eq!(below(3), ["X/Renewal/Snowflake"]);
    // the occurrences exist only as parents of the pages below them
    assert_eq!(groups[0]["occurrence_is_virtual"], true);
    assert_eq!(groups[2]["occurrence_is_virtual"], false); // `renewal` has its own file
    // several segments in a row
    let v = t.ok("segment_view", json!({"segment": "ey/renewal"}));
    assert_eq!(v["groups"].as_array().unwrap().len(), 1);
    assert_eq!(
        t.ok("segment_view", json!({"segment": "nothing here"}))["groups"],
        json!([])
    );
    // a page inside a path that was only linked, never written, is part of the hierarchy and a place
    // (`Napis/first/deepest` is italic in the hierarchy, `Napis/second/deeper/deepest` is a file)
    std::fs::create_dir_all(pages.join("Napis/second/deeper")).unwrap();
    std::fs::write(
        pages.join("Napis/second/deeper/deepest.md"),
        "- the real one\n",
    )
    .unwrap();
    std::fs::write(
        pages.join("Napis.md"),
        "- links to [[Napis/first/deepest]]\n",
    )
    .unwrap();
    for p in ["Napis/second/deeper/deepest", "Napis"] {
        t.ok("get_page", json!({"name": p}));
    }
    let n = t.ok("segment_places", json!({"segments": ["deepest"]}));
    assert_eq!(n["places"], json!([2]), "one file and one linked-only page");
    let v = t.ok("segment_view", json!({"segment": "deepest"}));
    let occ: Vec<&str> = v["groups"]
        .as_array()
        .unwrap()
        .iter()
        .map(|g| g["occurrence"].as_str().unwrap())
        .collect();
    assert_eq!(occ, ["Napis/first/deepest", "Napis/second/deeper/deepest"]);
    assert_eq!(v["groups"][0]["occurrence_is_virtual"], true);
    // places per segment, for the badge
    let n = t.ok(
        "segment_places",
        json!({"segments": ["Renewal", "Siemens Energy", "P", "nothing"]}),
    );
    assert_eq!(n["places"], json!([4, 1, 1, 0]));
    // the query clause: tasks of every page with the segment in its path
    let q = t.ok(
        "run_query",
        json!({"text": "task: todo\npath: Renewal\nview: list"}),
    );
    let rows: Vec<&str> = q["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["page"].as_str().unwrap())
        .collect();
    assert_eq!(
        rows,
        [
            "P/Capgemini/Renewal/Siemens Energy",
            "P/EY/Renewal/APotheke",
            "X/Renewal/Snowflake"
        ],
        "{q}"
    );
    // negation and OR work like every other clause
    let q = t.ok(
        "run_query",
        json!({"text": "task: todo\npath: Renewal\n-path: Capgemini\nview: list"}),
    );
    assert_eq!(q["rows"].as_array().unwrap().len(), 2);
    let q = t.ok(
        "run_query",
        json!({"text": "task: todo\npath: Capgemini, Snowflake\nview: list"}),
    );
    assert_eq!(q["rows"].as_array().unwrap().len(), 2);
    // `%` and `_` in a name are plain characters
    let q = t.ok(
        "run_query",
        json!({"text": "task: todo\npath: 100%\nview: list"}),
    );
    assert!(q["errors"].as_array().unwrap().is_empty() && q["rows"].as_array().unwrap().is_empty());
}

/// LN-10: a linked reference carries its whole subtree, and a block below another referencing block of the same
/// page is part of that one instead of a second reference.
#[test]
fn linked_references_carry_their_subtree_and_skip_nested_ones() {
    let t = start("basic");
    let pages = t.graph().join("pages");
    std::fs::write(pages.join("Deal.md"), "- the deal\n").unwrap();
    std::fs::write(
        pages.join("meeting.md"),
        "- intro\n- [[Deal]]:\n  - discussion\n  - [[Deal]] again, nested\n    - deep note\n  - TODO call back\n- other [[Deal]] block\n",
    )
    .unwrap();
    t.ok("get_page", json!({"name": "meeting"}));
    let b = t.ok("backlinks", json!({"page": "Deal"}));
    let items: Vec<&Value> = b
        .as_array()
        .unwrap()
        .iter()
        .filter(|l| l["source_page"] == "meeting")
        .collect();
    assert_eq!(
        items.len(),
        2,
        "the nested [[Deal]] is not a reference of its own"
    );
    assert_eq!(items[0]["block_path"], json!([1]));
    let kids = items[0]["block"]["children"].as_array().unwrap();
    assert_eq!(kids.len(), 3, "discussion, the nested link, the todo");
    assert_eq!(
        kids[1]["children"].as_array().unwrap().len(),
        1,
        "deep note"
    );
    assert_eq!(items[1]["block_path"], json!([2]));
}

/// LN-11: a link (or tag) on a block applies to everything below it in queries; `direct: yes` turns that off; other
/// properties and dates are not inherited.
#[test]
fn links_and_tags_apply_to_the_subtree() {
    let t = start("basic");
    let pages = t.graph().join("pages");
    std::fs::write(pages.join("Client One.md"), "- the client\n").unwrap();
    std::fs::write(
        pages.join("notes.md"),
        "- unrelated\n  - TODO other task\n- [[Client One]]: sync\n  deadline:: 2026-12-01\n  priority:: high\n  - TODO child task\n    - TODO grandchild task\n  - plain note\n- #urgent block\n  - TODO tagged child\n",
    )
    .unwrap();
    t.ok("get_page", json!({"name": "notes"}));
    let rows = |text: &str| -> Vec<String> {
        let q = t.ok("run_query", json!({"text": text, "view": null}));
        assert_eq!(q["errors"], json!([]), "{text}");
        let mut v: Vec<String> = q["rows"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|r| r["page"] == "notes")
            .map(|r| {
                r["block"]["text"]
                    .as_str()
                    .unwrap()
                    .lines()
                    .next()
                    .unwrap()
                    .to_string()
            })
            .collect();
        v.sort();
        v
    };
    assert_eq!(
        rows("task: todo\nlinks: Client One"),
        ["TODO child task", "TODO grandchild task"]
    );
    assert!(rows("task: todo\nlinks: Client One\ndirect: yes").is_empty());
    assert_eq!(rows("task: todo\ntag: urgent"), ["TODO tagged child"]);
    // the block itself and every descendant, not its siblings
    assert_eq!(rows("links: Client One").len(), 4);
    assert_eq!(rows("links: Client One\ndirect: yes").len(), 1);
    // a deadline or priority on the parent does not make the children due
    assert!(rows("task: todo\ndeadline: 2026-12-01").is_empty());
    assert!(rows("task: todo\nprop.priority: high").is_empty());
    let bad = t.ok("run_query", json!({"text": "direct: maybe", "view": null}));
    assert_ne!(bad["errors"], json!([]));
}

/// PR-5: `inherit: key` lets a property apply to everything below the block that has it; the nearest block wins,
/// the page's frontmatter is the root, and `direct: yes` turns it off.
#[test]
fn inherit_clause_applies_properties_to_the_subtree() {
    let t = start("basic");
    let pages = t.graph().join("pages");
    std::fs::write(
        pages.join("props.md"),
        "---\ntype: client\n---\n- plain top\n- group\n  client:: [[Acme]]\n  - child\n    - grandchild\n  - other\n    client:: [[Beta]]\n    - below other\n",
    )
    .unwrap();
    t.ok("get_page", json!({"name": "props"}));
    let rows = |text: &str| -> Vec<String> {
        let q = t.ok("run_query", json!({"text": text, "view": null}));
        assert_eq!(q["errors"], json!([]), "{text}");
        let mut v: Vec<String> = q["rows"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|r| r["page"] == "props")
            .map(|r| {
                r["block"]["text"]
                    .as_str()
                    .unwrap()
                    .lines()
                    .next()
                    .unwrap()
                    .to_string()
            })
            .collect();
        v.sort();
        v
    };
    // without `inherit:` only the block itself
    assert_eq!(rows("prop.client: Acme"), ["group"]);
    assert_eq!(
        rows("prop.client: Acme\ninherit: client"),
        ["child", "grandchild", "group"]
    );
    // the nearest block wins
    assert_eq!(
        rows("prop.client: Beta\ninherit: client"),
        ["below other", "other"]
    );
    assert_eq!(
        rows("prop.client: Acme\ninherit: client\ndirect: yes"),
        ["group"]
    );
    // blocks with no value anywhere in their chain
    assert_eq!(rows("prop.client: none\ninherit: client"), ["plain top"]);
    // page frontmatter is the root
    assert_eq!(rows("prop.type: client").len(), 0);
    assert_eq!(rows("prop.type: client\ninherit: type").len(), 6);
    let bad = t.ok(
        "run_query",
        json!({"text": "inherit: Bad Key", "view": null}),
    );
    assert_ne!(bad["errors"], json!([]));
}

/// A journal day has `type: journal` unless its frontmatter sets a type: icon rules match it, the file is not
/// touched, and a day's own `type:` wins.
#[test]
fn journal_pages_have_a_default_type() {
    let t = start("basic");
    let journals = t.graph().join("journals/2026");
    std::fs::create_dir_all(&journals).unwrap();
    std::fs::write(journals.join("2026-10-10.md"), "- plain day\n").unwrap();
    std::fs::write(
        journals.join("2026-10-11.md"),
        "---\ntype: trip\n---\n- own type\n",
    )
    .unwrap();
    for p in ["2026-10-10", "2026-10-11"] {
        t.ok("get_page", json!({"name": p}));
    }
    t.ok(
        "set_settings",
        json!({"page_icons": [
            {"key": "type", "value": "journal", "icon": "🗓"},
            {"key": "type", "value": "trip", "icon": "🧳"}
        ]}),
    );
    let icons = t.ok("page_icons", json!({}));
    let get = |n: &str| {
        icons
            .as_array()
            .unwrap()
            .iter()
            .find(|i| i["name"] == n)
            .map(|i| i["icon"].as_str().unwrap().to_string())
    };
    assert_eq!(get("2026-10-10").as_deref(), Some("🗓"));
    assert_eq!(get("2026-10-11").as_deref(), Some("🧳"));
    // derived, never written
    assert_eq!(
        std::fs::read_to_string(journals.join("2026-10-10.md")).unwrap(),
        "- plain day\n"
    );
    // page queries see it too
    let q = t.ok(
        "run_query",
        json!({"text": "type: page\nprop.type: journal", "view": null}),
    );
    assert_eq!(q["errors"], json!([]));
    let names: Vec<_> = q["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["page"].as_str().unwrap().to_string())
        .collect();
    assert!(names.contains(&"2026-10-10".to_string()), "{names:?}");
    assert!(!names.contains(&"2026-10-11".to_string()), "{names:?}");
}

/// Today's journal has no file until something is written, but it gets the journal rule's icon like the days that
/// exist, so the feed's titles match.
#[test]
fn an_unwritten_today_gets_the_journal_icon() {
    let t = start_with("basic", false, "2026-11-20");
    t.ok(
        "set_settings",
        json!({"page_icons": [{"key": "type", "value": "journal", "icon": "🗓"}]}),
    );
    let icons = t.ok("page_icons", json!({}));
    let has = |n: &str| icons.as_array().unwrap().iter().any(|i| i["name"] == n);
    assert!(has("2026-11-20"), "{icons}");
    assert!(has("2026-10-01"), "{icons}");
}

/// BK-1..BK-5: snapshots through the API: default and custom folder, listing, restoring a folder and one page.
#[test]
fn backups_are_taken_listed_and_restored() {
    let t = start("basic");
    let s = t.ok("get_settings", json!({}));
    assert_eq!(s["backup_auto"], true);
    assert_eq!(s["backup_dir"], "");
    let l = t.ok("list_backups", json!({}));
    assert_eq!(l["backups"].as_array().unwrap().len(), 0);
    let default_dir = l["default_dir"].as_str().unwrap().to_string();
    assert_eq!(l["dir"], default_dir);
    assert!(
        !Path::new(&default_dir).starts_with(t.graph()),
        "the default folder is outside the graph"
    );
    // a custom folder: absolute, outside the graph
    let custom = t.tmp.path().join("my-backups");
    assert!(
        t.call("set_settings", json!({"backup_dir": "relative/x"}))
            .is_err()
    );
    assert!(
        t.call(
            "set_settings",
            json!({"backup_dir": t.graph().join("bk").to_str().unwrap()})
        )
        .is_err()
    );
    let s = t.ok(
        "set_settings",
        json!({"backup_dir": custom.to_str().unwrap(), "backup_auto": false}),
    );
    assert_eq!(s["backup_dir"], custom.to_str().unwrap());
    assert_eq!(s["backup_auto"], false);
    let l = t.ok("backup_now", json!({}));
    let b = l["backups"].as_array().unwrap();
    assert_eq!(b.len(), 1);
    assert_eq!(l["dir"], custom.to_str().unwrap());
    assert!(custom.join(b[0]["name"].as_str().unwrap()).exists());
    let name = b[0]["name"].as_str().unwrap().to_string();
    // restore everything into a new folder
    let target = t.tmp.path().join("restored");
    t.ok(
        "restore_backup",
        json!({"name": name, "target": target.to_str().unwrap()}),
    );
    assert_eq!(
        std::fs::read_to_string(target.join("pages/inbox.md")).unwrap(),
        t.read("pages/inbox.md")
    );
    // names that are paths are refused
    assert!(
        t.call(
            "restore_backup",
            json!({"name": "../x.zip", "target": t.tmp.path().join("y").to_str().unwrap()})
        )
        .is_err()
    );
    // one page back
    let before = t.read("pages/inbox.md");
    std::fs::write(t.graph().join("pages/inbox.md"), "- ruined\n").unwrap();
    let r = t.ok(
        "restore_backup_page",
        json!({"name": name, "page": "inbox"}),
    );
    assert_eq!(r["items"], json!(["inbox"]));
    assert_eq!(t.read("pages/inbox.md"), before);
    assert_eq!(
        t.ok("list_backups", json!({}))["backups"]
            .as_array()
            .unwrap()
            .len(),
        2,
        "the snapshot a page came from stays while it is used"
    );
}

/// A rename that touches several files and a delete of several pages first save a restore point (BK-6); the page
/// history lists, shows and restores versions (VH-1..VH-3).
#[test]
fn bulk_changes_take_a_restore_point_and_pages_have_a_history() {
    let t = start("basic");
    let custom = t.tmp.path().join("bk");
    t.ok(
        "set_settings",
        json!({"backup_dir": custom.to_str().unwrap()}),
    );
    let count = |t: &T| {
        t.ok("list_backups", json!({}))["backups"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|b| b["before"] == true)
            .count()
    };
    assert_eq!(count(&t), 0);
    // `people/anna` is linked from several pages: the rename rewrites them
    t.ok(
        "rename_apply",
        json!({"old": "people/anna", "new": "team/anna"}),
    );
    assert_eq!(count(&t), 1);
    // the restore point has the old name, the graph the new one
    let l = t.ok("list_backups", json!({}));
    let name = l["backups"][0]["name"].as_str().unwrap().to_string();
    assert!(name.ends_with("-before.zip"), "{name}");
    let target = t.tmp.path().join("old");
    t.ok(
        "restore_backup",
        json!({"name": name, "target": target.to_str().unwrap()}),
    );
    assert!(target.join("pages/people/anna.md").exists());
    assert!(!t.graph().join("pages/people/anna.md").exists());
    // with automatic backups off nothing is saved
    t.ok("set_settings", json!({"backup_auto": false}));
    t.ok(
        "rename_apply",
        json!({"old": "team/anna", "new": "people/anna"}),
    );
    assert_eq!(count(&t), 1);

    // page history
    let p = t.ok("get_page", json!({"name": "inbox"}));
    let hash = p["hash"].as_str().unwrap().to_string();
    let original = t.read("pages/inbox.md");
    assert_eq!(t.ok("page_versions", json!({"page": "inbox"})), json!([]));
    t.ok(
        "apply_ops",
        json!({"page": "inbox", "base_hash": hash, "ops": [
            {"op": "insert", "parent": [], "index": 0, "text": "history test"}
        ]}),
    );
    let v = t.ok("page_versions", json!({"page": "inbox"}));
    let v = v.as_array().unwrap();
    assert_eq!(v.len(), 2, "{v:?}");
    let oldest = v.last().unwrap()["id"].as_str().unwrap().to_string();
    let text = t.ok("page_version_text", json!({"page": "inbox", "id": oldest}));
    assert_eq!(text["value"], original);
    t.ok(
        "restore_page_version",
        json!({"page": "inbox", "id": oldest}),
    );
    assert_eq!(t.read("pages/inbox.md"), original);
    assert!(
        t.call("page_version_text", json!({"page": "inbox", "id": "../x"}))
            .is_err()
    );
}

/// Deleting a page moves it to the trash (outside the graph folder), links stay, and it can be restored; archived
/// pages are listed and their tasks stay off the agenda.
#[test]
fn delete_restore_and_archive_pages() {
    let t = start("basic");
    let pages = t.graph().join("pages");
    std::fs::create_dir_all(pages.join("proj")).unwrap();
    std::fs::write(pages.join("proj.md"), "- project home\n").unwrap();
    std::fs::write(pages.join("proj/a.md"), "- TODO task in a\n").unwrap();
    std::fs::write(pages.join("user.md"), "- see [[proj/a]]\n").unwrap();
    for p in ["proj", "proj/a", "user"] {
        t.ok("get_page", json!({"name": p}));
    }
    // delete a single page: file gone, link stays, one page links to it
    let d = t.ok(
        "delete_pages",
        json!({"page": "proj/a", "include_subpages": false}),
    );
    assert_eq!(d["deleted"], json!(["proj/a"]));
    assert_eq!(d["linked_from"], json!(["user"]));
    assert!(!pages.join("proj/a.md").exists());
    assert!(pages.join("proj.md").exists());
    assert!(
        !t.graph().join(".notatnik").join("trash").exists(),
        "the trash is not in the graph folder"
    );
    // restore brings the same text back
    let r = t.ok("restore_pages", json!({"trash_id": d["trash_id"]}));
    assert_eq!(r["items"], json!(["proj/a"]));
    assert_eq!(
        std::fs::read_to_string(pages.join("proj/a.md")).unwrap(),
        "- TODO task in a\n"
    );
    // a namespace with its subpages
    let d = t.ok(
        "delete_pages",
        json!({"page": "proj", "include_subpages": true}),
    );
    let mut names: Vec<String> = d["deleted"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    names.sort();
    assert_eq!(names, ["proj", "proj/a"]);
    assert!(!pages.join("proj.md").exists() && !pages.join("proj/a.md").exists());
    t.ok("restore_pages", json!({"trash_id": d["trash_id"]}));
    assert!(pages.join("proj.md").exists() && pages.join("proj/a.md").exists());
    // a page that does not exist
    let e = t
        .call(
            "delete_pages",
            json!({"page": "nope/nothing", "include_subpages": false}),
        )
        .unwrap_err();
    assert_eq!(e.code, "not_found");
    // archive: the property marks it; its tasks leave the agenda, those of pages below it too
    let before = t.ok("agenda", json!({"ns": null, "tag": null, "priority": null}));
    let count = |v: &Value| -> usize {
        ["overdue", "today_items", "undated"]
            .iter()
            .map(|k| v[k].as_array().unwrap().len())
            .sum::<usize>()
            + v["upcoming"]
                .as_array()
                .unwrap()
                .iter()
                .map(|d| d["items"].as_array().unwrap().len())
                .sum::<usize>()
    };
    assert!(before.to_string().contains("task in a"));
    std::fs::write(
        pages.join("proj.md"),
        "---\narchived: true\n---\n\n- project home\n",
    )
    .unwrap();
    t.ok("get_page", json!({"name": "proj"}));
    let arch = t.ok("archived_pages", json!({}));
    assert_eq!(arch["items"], json!(["proj"]));
    let after = t.ok("agenda", json!({"ns": null, "tag": null, "priority": null}));
    assert!(!after.to_string().contains("task in a"));
    assert_eq!(count(&after) + 1, count(&before));
}

/// TK-11: any number of priority labels; changing them re-reads the files, the `priority:` clause and the agenda
/// filter use the labels.
#[test]
fn priority_labels_are_configurable() {
    let t = start("basic");
    let pages = t.graph().join("pages");
    std::fs::write(
        pages.join("prio.md"),
        "- TODO first\n  priority:: A\n- TODO second\n  priority:: low\n- TODO third\n  priority:: P2\n- TODO none\n",
    )
    .unwrap();
    let texts = |q: &str| -> Vec<String> {
        let r = t.ok("run_query", json!({"text": q, "view": null}));
        assert_eq!(r["errors"], json!([]), "{q}");
        let mut v: Vec<String> = r["rows"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|x| x["page"] == "prio")
            .map(|x| {
                x["block"]["text"]
                    .as_str()
                    .unwrap()
                    .lines()
                    .next()
                    .unwrap()
                    .to_string()
            })
            .collect();
        v.sort();
        v
    };
    // default labels: A (high) and C (low); `P2` is unknown here
    t.ok("get_page", json!({"name": "prio"}));
    assert_eq!(texts("task: todo\npriority: A"), ["TODO first"]);
    assert_eq!(
        texts("task: todo\npriority: a, c"),
        ["TODO first", "TODO second"]
    );
    let p = t.ok("get_page", json!({"name": "prio"}));
    assert_eq!(p["blocks"][1]["task"]["priority"], "C");
    assert!(
        p["diagnostics"].to_string().contains("priority"),
        "P2 is not a label yet"
    );
    // four labels: `low` is now the fourth, `P2` the second; A is no label any more
    let s = t.ok(
        "set_settings",
        json!({"theme": null, "date_locale": null, "link_brackets": null, "date_format": null,
               "date_pattern": null, "week_start": null, "completed_log_limit": null,
               "priorities": ["P1", "P2", "P3", "P4"], "page_icons": null}),
    );
    assert_eq!(s["priorities"], json!(["P1", "P2", "P3", "P4"]));
    assert_eq!(
        t.ok("graph_info", json!({}))["priority_labels"],
        json!(["P1", "P2", "P3", "P4"])
    );
    let p = t.ok("get_page", json!({"name": "prio"}));
    let prio = |i: usize| {
        (
            p["blocks"][i]["task"]["priority"].clone(),
            p["blocks"][i]["task"]["priority_rank"].clone(),
        )
    };
    assert_eq!(
        prio(1),
        json!(["P4", 3])
            .as_array()
            .map(|a| (a[0].clone(), a[1].clone()))
            .unwrap()
    );
    assert_eq!(prio(2), (json!("P2"), json!(1)));
    assert_eq!(
        texts("task: todo\npriority: P2, P4"),
        ["TODO second", "TODO third"]
    );
    // sorted by priority: P2 (third), then P4 (second)
    let r = t.ok(
        "run_query",
        json!({"text": "task: todo\npriority: P2, P4\nsort: priority asc", "view": null}),
    );
    let order: Vec<&str> = r["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x["block"]["text"].as_str().unwrap().lines().next().unwrap())
        .collect();
    assert_eq!(order[..2], ["TODO third", "TODO second"]);
    // a label that does not exist is refused by the settings and by the agenda filter
    assert!(
        t.call("set_settings", json!({"theme": null, "date_locale": null, "link_brackets": null,
            "date_format": null, "date_pattern": null, "week_start": null, "completed_log_limit": null,
            "priorities": ["x", "X"], "page_icons": null})).is_err()
    );
    assert!(
        t.call(
            "agenda",
            json!({"ns": null, "tag": null, "priority": "nope"})
        )
        .is_err()
    );
    assert!(
        t.call("agenda", json!({"ns": null, "tag": null, "priority": "p2"}))
            .is_ok()
    );
}

/// PR-10: setting `type` adds the properties (and, for an empty page, the blocks) of `templates/<type>.md`.
#[test]
fn setting_type_applies_the_template_of_that_type() {
    let t = start("basic");
    std::fs::create_dir_all(t.graph().join("templates")).unwrap();
    std::fs::write(
        t.graph().join("templates/Person.md"),
        "---\ntype: person\nimportance: 1\npartner:\nadded: {{date}}\n---\n- Contact {{title}}\n  - Phone\n- Notes\n",
    )
    .unwrap();
    // a new page: properties and blocks, one write
    let r = t.ok(
        "apply_ops",
        json!({"page": "people/Jan", "base_hash": null,
               "ops": [{"op": "set_page_property", "key": "type", "value": "person"}]}),
    );
    assert_eq!(r["template"], "Person");
    assert_eq!(
        r["template_added"],
        json!(["importance", "partner", "added"])
    );
    let text = t.read("pages/people/Jan.md");
    assert!(
        text.starts_with("---\ntype: person\nimportance: 1\npartner:\nadded: 2026-10-03\n---\n"),
        "{text}"
    );
    assert!(
        text.contains("- Contact Jan\n  - Phone\n- Notes\n"),
        "{text}"
    );
    // a page with its own value and blocks: only the missing properties, no blocks
    let p = t.ok("get_page", json!({"name": "inbox"}));
    let hash = p["hash"].as_str().unwrap().to_string();
    let before = t.read("pages/inbox.md");
    let r = t.ok(
        "apply_ops",
        json!({"page": "inbox", "base_hash": hash, "ops": [
            {"op": "set_page_property", "key": "importance", "value": "5"},
            {"op": "set_page_property", "key": "type", "value": "person"}]}),
    );
    assert_eq!(r["template_added"], json!(["partner", "added"]));
    let after = t.read("pages/inbox.md");
    assert!(after.contains("importance: 5"), "{after}");
    assert!(!after.contains("Contact"), "{after}");
    assert!(after.contains(before.lines().last().unwrap()), "{after}");
    // the info and the explicit action
    let info = t.ok("template_info", json!({"page": "people/Jan"}));
    assert_eq!(info, json!({"template": "Person", "missing": []}));
    let hash = t.ok("get_page", json!({"name": "inbox"}))["hash"].clone();
    t.ok(
        "apply_ops",
        json!({"page": "inbox", "base_hash": hash, "ops": [{"op": "remove_page_property", "key": "partner"}]}),
    );
    let info = t.ok("template_info", json!({"page": "inbox"}));
    assert_eq!(info["missing"], json!(["partner"]));
    let r = t.ok("apply_template", json!({"page": "inbox"}));
    assert_eq!(r["template_added"], json!(["partner"]));
    // turned off: setting `type` changes nothing else
    t.ok("set_settings", json!({"templates_auto": false}));
    let r = t.ok(
        "apply_ops",
        json!({"page": "people/Eva", "base_hash": null,
               "ops": [{"op": "set_page_property", "key": "type", "value": "person"}]}),
    );
    assert_eq!(r["template"], Value::Null);
    assert_eq!(t.read("pages/people/Eva.md"), "---\ntype: person\n---\n");
    // no template for the type: nothing happens, and the explicit action says so
    t.ok(
        "apply_ops",
        json!({"page": "people/Odd", "base_hash": null,
               "ops": [{"op": "set_page_property", "key": "type", "value": "gadget"}]}),
    );
    assert!(
        t.call("apply_template", json!({"page": "people/Odd"}))
            .is_err()
    );
}

/// PR-10: a page becomes the template of its type; listing, replacing and deleting templates.
#[test]
fn a_page_is_saved_as_the_template_of_its_type() {
    let t = start("basic");
    t.ok(
        "apply_ops",
        json!({"page": "people/Model", "base_hash": null, "ops": [
            {"op": "set_page_property", "key": "type", "value": "person"},
            {"op": "set_page_property", "key": "importance", "value": "3"},
            {"op": "set_page_property", "key": "title", "value": "Model Person"},
            {"op": "insert", "parent": [], "index": 0, "text": "Contact"},
            {"op": "insert", "parent": [0], "index": 0, "text": "Phone"}]}),
    );
    // a page without a type cannot be a template
    assert!(
        t.call(
            "save_template",
            json!({"page": "inbox", "keep_values": true, "replace": false})
        )
        .is_err()
    );
    let e = t.ok(
        "save_template",
        json!({"page": "people/Model", "keep_values": false, "replace": false}),
    );
    assert_eq!(
        e,
        json!({"name": "person", "fields": ["importance"], "blocks": 2})
    );
    assert_eq!(
        t.read("templates/person.md"),
        "---\ntype: person\nimportance:\n---\n- Contact\n  - Phone\n"
    );
    // not replaced without asking
    let err = t
        .call(
            "save_template",
            json!({"page": "people/Model", "keep_values": true, "replace": false}),
        )
        .unwrap_err();
    assert_eq!(err.code, "exists");
    t.ok(
        "save_template",
        json!({"page": "people/Model", "keep_values": true, "replace": true}),
    );
    assert!(t.read("templates/person.md").contains("importance: 3\n"));
    // a new page of that type starts from it
    let r = t.ok(
        "apply_ops",
        json!({"page": "people/New", "base_hash": null,
               "ops": [{"op": "set_page_property", "key": "type", "value": "person"}]}),
    );
    assert_eq!(r["template"], "person");
    assert!(
        t.read("pages/people/New.md")
            .contains("- Contact\n  - Phone\n")
    );
    // the list, and deleting
    let l = t.ok("list_templates", json!({}));
    assert_eq!(l[0]["name"], "person");
    let l = t.ok("delete_template", json!({"name": "person"}));
    assert_eq!(l, json!([]));
    assert!(!t.graph().join("templates/person.md").exists());
    assert!(t.call("delete_template", json!({"name": "../x"})).is_err());
}

/// NS-10, JR-6, PR-10: templates of a namespace and of new journal days; `/template` blocks; restoring a deleted one.
#[test]
fn namespace_and_journal_templates_apply_to_new_pages() {
    let t = start("basic");
    std::fs::create_dir_all(t.graph().join("templates")).unwrap();
    std::fs::write(
        t.graph().join("templates/person.md"),
        "---\ntype: person\nimportance: 1\n---\n- Contact {{title}}\n  - Phone\n",
    )
    .unwrap();
    std::fs::write(
        t.graph().join("templates/journal.md"),
        "---\nmood:\n---\n- Plan for {{date}}\n- Notes\n",
    )
    .unwrap();
    let s = t.ok(
        "set_settings",
        json!({"journal_template": "journal",
               "namespace_templates": [{"namespace": "people", "template": "person"}]}),
    );
    assert_eq!(s["journal_template"], "journal");
    assert_eq!(
        s["namespace_templates"],
        json!([{"namespace": "people", "template": "person"}])
    );
    assert!(
        t.read(".notatnik/config.toml")
            .contains("templates/person.md")
    );
    // a new page below the namespace: type, properties and blocks, and the typed first block comes after them
    let r = t.ok(
        "apply_ops",
        json!({"page": "people/Jan", "base_hash": null,
               "ops": [{"op": "insert", "parent": [], "index": 0, "text": "my note"}]}),
    );
    assert_eq!(r["template"], "person");
    assert_eq!(r["template_added"], json!(["type", "importance"]));
    let text = t.read("pages/people/Jan.md");
    assert!(
        text.starts_with("---\ntype: person\nimportance: 1\n---\n"),
        "{text}"
    );
    assert!(
        text.ends_with("- Contact Jan\n  - Phone\n- my note\n"),
        "{text}"
    );
    // the namespace page itself, and a page elsewhere, are not touched
    t.ok(
        "apply_ops",
        json!({"page": "people", "base_hash": null,
               "ops": [{"op": "insert", "parent": [], "index": 0, "text": "all people"}]}),
    );
    assert_eq!(t.read("pages/people.md"), "- all people\n");
    // a new journal day
    let r = t.ok(
        "apply_ops",
        json!({"page": "2026-10-20", "base_hash": null,
               "ops": [{"op": "insert", "parent": [], "index": 0, "text": "typed"}]}),
    );
    assert_eq!(r["template"], "journal");
    let text = t.read("journals/2026/2026-10-20.md");
    assert!(text.contains("mood:"), "{text}");
    assert!(
        text.contains("- Plan for 2026-10-20\n- Notes\n- typed\n"),
        "{text}"
    );
    // `/template` gets the blocks with the variables of the page it is inserted into
    let b = t.ok(
        "template_blocks",
        json!({"name": "person", "page": "people/Eva"}),
    );
    assert_eq!(
        b["blocks"],
        json!([
            {"parent": [], "index": 0, "text": "Contact Eva"},
            {"parent": [0], "index": 0, "text": "Phone"}
        ])
    );
    // and the page properties it lacks: the template's own type and its fields, empty ones stay empty
    assert_eq!(
        b["props"],
        json!([
            {"op": "set_page_property", "key": "type", "value": "person"},
            {"op": "set_page_property", "key": "importance", "value": "1"}
        ])
    );
    // deleted templates can be listed and restored; not over an existing one
    t.ok("delete_template", json!({"name": "person"}));
    let d = t.ok("deleted_templates", json!({}));
    assert_eq!(d[0]["name"], "person");
    let id = d[0]["id"].clone();
    let l = t.ok("restore_template", json!({"id": id, "name": "person"}));
    assert_eq!(l.as_array().unwrap().len(), 2);
    assert!(t.graph().join("templates/person.md").exists());
    assert_eq!(t.ok("deleted_templates", json!({})), json!([]));
    // turned off: new pages stay as they are
    t.ok("set_settings", json!({"templates_auto": false}));
    t.ok(
        "apply_ops",
        json!({"page": "people/Zed", "base_hash": null,
               "ops": [{"op": "insert", "parent": [], "index": 0, "text": "x"}]}),
    );
    assert_eq!(t.read("pages/people/Zed.md"), "- x\n");
}

/// P5.11: a template that cannot be read inserts nothing and says why.
#[test]
fn a_broken_template_inserts_nothing_and_reports_it() {
    let t = start("basic");
    std::fs::create_dir_all(t.graph().join("templates")).unwrap();
    std::fs::write(
        t.graph().join("templates/person.md"),
        "---\na: [unclosed\n---\n- Contact\n",
    )
    .unwrap();
    // the user's own edit goes through; nothing of the template, and the reason is reported
    let r = t.ok(
        "apply_ops",
        json!({"page": "people/Bad", "base_hash": null,
               "ops": [{"op": "set_page_property", "key": "type", "value": "person"}]}),
    );
    assert!(
        r["template_error"]
            .as_str()
            .unwrap()
            .contains("templates/person.md"),
        "{r}"
    );
    assert_eq!(r["template"], Value::Null);
    assert_eq!(t.read("pages/people/Bad.md"), "---\ntype: person\n---\n");
    let err = t
        .call("apply_template", json!({"page": "people/Bad"}))
        .unwrap_err();
    assert_eq!(err.code, "invalid");
}
