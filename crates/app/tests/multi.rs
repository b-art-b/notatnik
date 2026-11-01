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

// ---- moving notes between graphs (MG-2 to MG-4) -----------------------------------------------------

/// Writes a page file and lets the index see it.
fn put(w: &W, slot: &str, root: &Path, name: &str, rel: &str, text: &str) {
    let abs = root.join(rel);
    std::fs::create_dir_all(abs.parent().unwrap()).unwrap();
    std::fs::write(abs, text).unwrap();
    w.ok(slot, "get_page", json!({"name": name}));
}

/// Arguments of a transfer of page `page` from the first graph to the one in slot 1.
fn pages(page: &str) -> Value {
    json!({"from": "", "to_slot": "1", "kind": "pages", "page": page, "include_subpages": false,
           "remove": false, "clash": "keep_both"})
}

fn with(mut v: Value, key: &str, val: Value) -> Value {
    v[key] = val;
    v
}

fn two() -> (W, PathBuf, PathBuf) {
    let w = w();
    let a = w.open("", "from-graph");
    let b = w.open("1", "to-graph");
    (w, a, b)
}

#[test]
fn a_page_is_copied_and_the_source_keeps_it() {
    let (w, a, b) = two();
    let text =
        "---\ntitle: Alpha\n---\n- first ^abc123\n  - child\n- see [[Other]] and ((abc123))\n";
    put(&w, "", &a, "alpha", "pages/alpha.md", text);
    let plan = w.ok("", "transfer_plan", pages("alpha"));
    assert_eq!(plan["items"][0]["action"], "create");
    assert_eq!(plan["from_name"], "from-graph");
    assert_eq!(plan["to_name"], "to-graph");
    assert_eq!(
        plan["missing_links"],
        json!(["Other"]),
        "the target has no page Other"
    );
    assert_eq!(plan["dangling_refs"], 0);
    assert!(!b.join("pages/alpha.md").exists(), "a plan writes nothing");

    let r = w.ok("", "transfer_apply", pages("alpha"));
    assert_eq!(r["written"], json!(["alpha"]));
    assert_eq!(
        read(&b, "pages/alpha.md"),
        text,
        "the text is copied as it is"
    );
    assert!(a.join("pages/alpha.md").exists());
    assert!(
        w.names("1").contains(&"alpha".to_string()),
        "the target indexed it"
    );
    assert_eq!(
        w.ok("1", "get_page", json!({"name": "alpha"}))["title"],
        "Alpha"
    );
}

#[test]
fn subpages_come_along_and_a_clash_keeps_both() {
    let (w, a, b) = two();
    put(&w, "", &a, "proj", "pages/proj.md", "- root [[proj/sub]]\n");
    put(
        &w,
        "",
        &a,
        "proj/sub",
        "pages/proj/sub.md",
        "- sub, back to [[proj]]\n",
    );
    put(&w, "1", &b, "proj", "pages/proj.md", "- the target's own\n");

    let args = with(pages("proj"), "include_subpages", json!(true));
    let plan = w.ok("", "transfer_plan", args.clone());
    let targets: Vec<&str> = plan["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["target"].as_str().unwrap())
        .collect();
    assert_eq!(
        targets,
        ["proj (from from-graph)", "proj (from from-graph)/sub"]
    );
    w.ok("", "transfer_apply", args);
    assert_eq!(
        read(&b, "pages/proj.md"),
        "- the target's own\n",
        "nothing of the target changes"
    );
    assert_eq!(
        read(&b, "pages/proj (from from-graph).md"),
        "- root [[proj (from from-graph)/sub]]\n",
        "links inside the copy follow the new names"
    );
    assert_eq!(
        read(&b, "pages/proj (from from-graph)/sub.md"),
        "- sub, back to [[proj (from from-graph)]]\n"
    );
}

#[test]
fn a_clash_can_be_appended_or_skipped() {
    let (w, a, b) = two();
    put(
        &w,
        "",
        &a,
        "note",
        "pages/note.md",
        "- from a\n  - deeper\n",
    );
    put(&w, "1", &b, "note", "pages/note.md", "- from b\n");

    let skip = with(pages("note"), "clash", json!("skip"));
    let r = w.ok("", "transfer_apply", skip);
    assert_eq!(r["written"], json!([]));
    assert_eq!(read(&b, "pages/note.md"), "- from b\n");

    let append = with(pages("note"), "clash", json!("append"));
    let plan = w.ok("", "transfer_plan", append.clone());
    assert_eq!(plan["items"][0]["action"], "append");
    w.ok("", "transfer_apply", append);
    assert_eq!(
        read(&b, "pages/note.md"),
        "- from b\n- from a\n  - deeper\n"
    );
}

#[test]
fn a_journal_day_is_appended_to_the_same_day() {
    let (w, a, b) = two();
    put(
        &w,
        "",
        &a,
        "2026-10-03",
        "journals/2026/2026-10-03.md",
        "- from a\n",
    );
    put(
        &w,
        "1",
        &b,
        "2026-10-03",
        "journals/2026/2026-10-03.md",
        "- from b\n",
    );
    let r = w.ok("", "transfer_apply", pages("2026-10-03"));
    assert_eq!(r["written"], json!(["2026-10-03"]));
    assert_eq!(
        read(&b, "journals/2026/2026-10-03.md"),
        "- from b\n- from a\n"
    );
}

#[test]
fn pictures_travel_with_the_text() {
    let (w, a, b) = two();
    std::fs::create_dir_all(a.join("assets/2026/10")).unwrap();
    std::fs::write(a.join("assets/2026/10/same-11111111.png"), b"same").unwrap();
    std::fs::write(a.join("assets/2026/10/other-22222222.png"), b"from a").unwrap();
    std::fs::write(a.join("assets/2026/10/new-33333333.png"), b"new").unwrap();
    std::fs::create_dir_all(b.join("assets/2026/10")).unwrap();
    std::fs::write(b.join("assets/2026/10/same-11111111.png"), b"same").unwrap();
    std::fs::write(b.join("assets/2026/10/other-22222222.png"), b"from b").unwrap();
    let text = "- ![](../../assets/2026/10/same-11111111.png)\n- ![](../../assets/2026/10/other-22222222.png)\n- ![](../../assets/2026/10/new-33333333.png)\n- ![](../../assets/2026/10/gone-44444444.png)\n";
    // the page sits one folder deeper in the target: a namespace
    put(
        &w,
        "",
        &a,
        "img",
        "pages/img.md",
        &text.replace("../../", "../"),
    );

    let plan = w.ok("", "transfer_plan", pages("img"));
    let action = |p: &str| -> String {
        plan["assets"]
            .as_array()
            .unwrap()
            .iter()
            .find(|x| x["path"] == p)
            .unwrap()["action"]
            .as_str()
            .unwrap()
            .to_string()
    };
    assert_eq!(action("assets/2026/10/same-11111111.png"), "reuse");
    assert_eq!(action("assets/2026/10/other-22222222.png"), "rename");
    assert_eq!(action("assets/2026/10/new-33333333.png"), "copy");
    assert_eq!(action("assets/2026/10/gone-44444444.png"), "missing");

    w.ok("", "transfer_apply", pages("img"));
    let got = read(&b, "pages/img.md");
    assert!(
        got.contains("![](../assets/2026/10/same-11111111.png)"),
        "{got}"
    );
    assert!(
        !got.contains("other-22222222.png"),
        "the clashing name was changed: {got}"
    );
    assert!(got.contains("![](../assets/2026/10/new-33333333.png)"));
    assert!(
        got.contains("gone-44444444.png"),
        "a missing file stays as it was written"
    );
    // the files: the same one kept, the other one next to it, the new one copied
    assert_eq!(
        std::fs::read(b.join("assets/2026/10/other-22222222.png")).unwrap(),
        b"from b"
    );
    let renamed = got
        .split("../assets/2026/10/")
        .nth(2)
        .and_then(|s| s.split(')').next())
        .unwrap();
    assert!(
        renamed.starts_with("other-22222222-") && renamed.ends_with(".png"),
        "{renamed}"
    );
    assert_eq!(
        std::fs::read(b.join("assets/2026/10").join(renamed)).unwrap(),
        b"from a"
    );
    assert_eq!(
        std::fs::read(b.join("assets/2026/10/new-33333333.png")).unwrap(),
        b"new"
    );
}

#[test]
fn a_block_id_the_target_uses_is_replaced_and_references_follow() {
    let (w, a, b) = two();
    put(
        &w,
        "",
        &a,
        "ids",
        "pages/ids.md",
        "- the block ^abc123\n- points at ((abc123)) and ((zzzzzz))\n",
    );
    put(
        &w,
        "1",
        &b,
        "elsewhere",
        "pages/elsewhere.md",
        "- taken ^abc123\n",
    );
    let plan = w.ok("", "transfer_plan", pages("ids"));
    assert_eq!(
        plan["dangling_refs"], 1,
        "((zzzzzz)) points at nothing that goes over"
    );
    w.ok("", "transfer_apply", pages("ids"));
    let got = read(&b, "pages/ids.md");
    assert!(!got.contains("abc123"), "{got}");
    let id = got
        .lines()
        .next()
        .unwrap()
        .rsplit('^')
        .next()
        .unwrap()
        .to_string();
    assert_eq!(id.len(), 6);
    assert!(got.contains(&format!("(({id}))")), "{got}");
    assert!(
        read(&b, "pages/elsewhere.md").contains("^abc123"),
        "the target's own block keeps its id"
    );
}

#[test]
fn moving_takes_the_page_out_of_the_source_and_can_be_undone() {
    let (w, a, b) = two();
    put(&w, "", &a, "mover", "pages/mover.md", "- text\n");
    put(&w, "", &a, "linker", "pages/linker.md", "- see [[mover]]\n");
    let args = with(pages("mover"), "remove", json!(true));
    let plan = w.ok("", "transfer_plan", args.clone());
    assert_eq!(plan["left_behind"], json!(["linker"]));
    let r = w.ok("", "transfer_apply", args);
    assert_eq!(r["undoable"], true);
    assert!(!a.join("pages/mover.md").exists());
    assert!(b.join("pages/mover.md").exists());

    let left = w.ok("", "transfer_undo", json!({}));
    assert_eq!(left["items"], json!([]));
    assert!(a.join("pages/mover.md").exists(), "back in the source");
    assert!(!b.join("pages/mover.md").exists(), "gone from the target");
    assert_eq!(
        w.call("", "transfer_undo", json!({})).unwrap_err().code,
        "invalid",
        "only once"
    );
}

#[test]
fn undoing_an_append_takes_the_blocks_out_again_unless_the_page_changed() {
    let (w, a, b) = two();
    put(&w, "", &a, "note", "pages/note.md", "- from a\n");
    put(&w, "1", &b, "note", "pages/note.md", "- from b\n");
    let append = with(pages("note"), "clash", json!("append"));
    w.ok("", "transfer_apply", append.clone());
    w.ok("", "transfer_undo", json!({}));
    assert_eq!(read(&b, "pages/note.md"), "- from b\n");

    w.ok("", "transfer_apply", append);
    let now = read(&b, "pages/note.md");
    put(
        &w,
        "1",
        &b,
        "note",
        "pages/note.md",
        &format!("{now}- typed later\n"),
    );
    let left = w.ok("", "transfer_undo", json!({}));
    assert_eq!(left["items"].as_array().unwrap().len(), 1, "{left}");
    assert!(
        read(&b, "pages/note.md").contains("typed later"),
        "the later edit is not lost"
    );
}

#[test]
fn blocks_go_into_a_new_or_an_existing_page_and_can_be_moved() {
    let (w, a, b) = two();
    put(
        &w,
        "",
        &a,
        "src",
        "pages/src.md",
        "- one\n- two ^two222\n  - two-a\n- three\n",
    );
    let blocks = |paths: Value, target: &str, remove: bool| {
        json!({"from": "", "to_slot": "1", "kind": "blocks", "page": "src", "blocks": paths,
               "target_page": target, "remove": remove, "clash": "keep_both"})
    };
    // a copy of the second block with its child into a page that does not exist
    w.ok("", "transfer_apply", blocks(json!([[1]]), "dest", false));
    assert_eq!(read(&b, "pages/dest.md"), "- two ^two222\n  - two-a\n");
    assert!(read(&a, "pages/src.md").contains("two-a"));
    // a move of the first and the third into the existing page
    w.ok(
        "",
        "transfer_apply",
        blocks(json!([[0], [2]]), "dest", true),
    );
    assert_eq!(
        read(&b, "pages/dest.md"),
        "- two ^two222\n  - two-a\n- one\n- three\n"
    );
    assert_eq!(read(&a, "pages/src.md"), "- two ^two222\n  - two-a\n");
    // and back
    w.ok("", "transfer_undo", json!({}));
    assert_eq!(
        read(&a, "pages/src.md"),
        "- one\n- two ^two222\n  - two-a\n- three\n"
    );
    assert_eq!(read(&b, "pages/dest.md"), "- two ^two222\n  - two-a\n");
}

#[test]
fn a_graph_that_is_not_open_gets_the_notes_too() {
    let w = w();
    let a = w.open("", "from-graph");
    let far = w.tmp.path().join("far-graph");
    {
        // made and closed again: a graph on this computer that no pane shows
        w.open("9", "far-graph");
        w.ok("", "close_slot", json!({"slot": "9"}));
    }
    put(&w, "", &a, "p", "pages/p.md", "- hello\n");
    let args = json!({"from": "", "to_path": far.to_str().unwrap(), "kind": "pages", "page": "p",
                      "remove": false, "clash": "keep_both"});
    let r = w.ok("", "transfer_apply", args);
    assert_eq!(r["plan"]["to_name"], "far-graph");
    assert_eq!(read(&far, "pages/p.md"), "- hello\n");
    // the same graph is refused
    let same = json!({"from": "", "to_path": a.to_str().unwrap(), "kind": "pages", "page": "p",
                      "remove": false, "clash": "keep_both"});
    assert_eq!(
        w.call("", "transfer_apply", same).unwrap_err().code,
        "invalid"
    );
    // and so is a folder that is no graph
    let none = json!({"from": "", "to_path": w.tmp.path().to_str().unwrap(), "kind": "pages", "page": "p",
                      "remove": false, "clash": "keep_both"});
    assert_eq!(
        w.call("", "transfer_apply", none).unwrap_err().code,
        "invalid"
    );
}

#[test]
fn encryption_only_warns_it_never_stops_a_transfer() {
    let (w, a, b) = two();
    // the first graph encrypts everything (a vault with scope all), the second has a server and no encryption
    w.ok(
        "",
        "encryption_setup",
        json!({"password": "correct horse battery"}),
    );
    w.ok("", "encryption_confirm", json!({}));
    let dirs = Dirs::under(&w.tmp.path().join("device"));
    let mut dev = notatnik_graph::DeviceSettings::load(&dirs).unwrap();
    let id_b = w.ok("1", "graph_info", json!({}))["id"]
        .as_str()
        .unwrap()
        .to_string();
    dev.set_remote(
        &id_b,
        Some(notatnik_graph::RemoteConfig {
            url: "https://cloud.example.org/remote.php/dav/files/me/b".into(),
            user: "me".into(),
            trust_folder_etags: false,
            merge: true,
        }),
    );
    dev.save(&dirs).unwrap();
    put(&w, "", &a, "secret", "pages/secret.md", "- private\n");
    let plan = w.ok("", "transfer_plan", pages("secret"));
    assert_eq!(plan["items"][0]["encrypted_in_source"], true);
    assert_eq!(plan["items"][0]["encrypted_in_target"], false);
    let warning = plan["warnings"][0].as_str().unwrap();
    assert!(
        warning.contains("plain text") && warning.contains("to-graph"),
        "{warning}"
    );
    w.ok("", "transfer_apply", pages("secret"));
    assert_eq!(read(&b, "pages/secret.md"), "- private\n");

    // the other way: the second graph is not encrypted, the first is; the first is locked
    w.ok("", "encryption_lock", json!({}));
    put(&w, "1", &b, "open", "pages/open.md", "- plain\n");
    let back = json!({"from": "1", "to_slot": "", "kind": "pages", "page": "open",
                      "remove": false, "clash": "keep_both"});
    let plan = w.ok("1", "transfer_plan", back.clone());
    assert_eq!(plan["target_locked"], true);
    w.ok("1", "transfer_apply", back);
    assert_eq!(
        read(&a, "pages/open.md"),
        "- plain\n",
        "written although the key is locked"
    );
}

// ---- which graph is this (MG-5) ---------------------------------------------------------------------

#[test]
fn a_graph_has_a_color_that_is_chosen_or_comes_from_its_id() {
    let (w, a, _b) = two();
    let info = w.ok("", "graph_info", json!({}));
    let id = info["id"].as_str().unwrap().to_string();
    let first = info["color"].as_u64().unwrap();
    assert!(first < 8);
    // the list of graphs and the list of open graphs say the same
    let list = w.ok("", "list_graphs", json!({}));
    let mine = list
        .as_array()
        .unwrap()
        .iter()
        .find(|g| g["id"] == id.as_str())
        .unwrap();
    assert_eq!(mine["color"].as_u64().unwrap(), first);
    assert_eq!(
        w.ok("", "slots", json!({}))[0]["color"].as_u64().unwrap(),
        first
    );

    // a chosen color stays, also when the graph is opened again
    let other = (first + 3) % 8;
    w.ok("", "graph_color_set", json!({"id": id, "color": other}));
    assert_eq!(
        w.ok("", "graph_info", json!({}))["color"].as_u64().unwrap(),
        other
    );
    w.ok(
        "",
        "open_graph",
        json!({"path": a.to_str().unwrap(), "create": false}),
    );
    assert_eq!(
        w.ok("", "graph_info", json!({}))["color"].as_u64().unwrap(),
        other
    );
    // and the way back to the color of the ID
    w.ok("", "graph_color_set", json!({"id": id, "color": null}));
    assert_eq!(
        w.ok("", "graph_info", json!({}))["color"].as_u64().unwrap(),
        first
    );
    // what is not in the palette is refused
    for bad in [json!(8), json!(300), json!(-1)] {
        assert_eq!(
            w.call("", "graph_color_set", json!({"id": id, "color": bad}))
                .unwrap_err()
                .code,
            "invalid"
        );
    }
}

// ---- two programs on one graph ----------------------------------------------------------------------------------

/// An app as the desktop shell runs it: the file watcher is on, so the graph is locked while it is open.
fn real_app(tmp: &Path, name: &str) -> AppState {
    let today = NaiveDate::parse_from_str("2026-10-03", "%Y-%m-%d").unwrap();
    AppState::new(AppOptions {
        dirs: Dirs::under(&tmp.join(name)),
        emit: Arc::new(|_, _| {}),
        today: Arc::new(move || today),
        watch: true,
    })
}

#[test]
fn a_graph_open_in_one_app_cannot_be_opened_by_another_until_it_is_closed() {
    let tmp = tempfile::tempdir().unwrap();
    let graph = tmp.path().join("shared-graph");
    let open = |app: &AppState| {
        dispatch_slot(
            app,
            "",
            "open_graph",
            json!({"path": graph.to_str().unwrap(), "create": true}),
        )
    };
    // two apps with their own device folders (the installed app and the dev app)
    let first = real_app(tmp.path(), "device-a");
    let second = real_app(tmp.path(), "device-b");
    open(&first).unwrap();
    let err = open(&second).unwrap_err();
    assert_eq!(err.code, "locked");
    // (both apps are this process here, so the message is the one for a graph that is still being closed)
    assert!(err.message.contains("closed"), "{}", err.message);
    // the same app opening the graph again (a reload) keeps its lock
    open(&first).unwrap();
    assert_eq!(open(&second).unwrap_err().code, "locked");
    // the second app can open another graph, and this one when the first app is gone
    drop(first);
    open(&second).unwrap();
}

#[test]
fn a_closed_second_graph_is_free_for_another_app() {
    let tmp = tempfile::tempdir().unwrap();
    let (a, b) = (tmp.path().join("graph-a"), tmp.path().join("graph-b"));
    let app = real_app(tmp.path(), "device-a");
    let other = real_app(tmp.path(), "device-b");
    let open = |app: &AppState, slot: &str, p: &Path| {
        dispatch_slot(
            app,
            slot,
            "open_graph",
            json!({"path": p.to_str().unwrap(), "create": true}),
        )
    };
    open(&app, "", &a).unwrap();
    open(&app, "1", &b).unwrap();
    assert_eq!(
        open(&other, "", &b).unwrap_err().code,
        "locked",
        "the second graph is locked too"
    );
    dispatch_slot(&app, "", "close_slot", json!({"slot": "1"})).unwrap();
    // the closed graph finishes its last sync on a thread of its own, then lets go
    let mut opened = false;
    for _ in 0..100 {
        if open(&other, "", &b).is_ok() {
            opened = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert!(opened, "the lock was not released");
}

/// The backup folder and the backup encryption belong to a graph: setting them in one leaves the other alone.
#[test]
fn backup_folder_and_encryption_belong_to_each_graph() {
    let w = w();
    w.open("", "alpha-graph");
    w.open("1", "beta-graph");
    let dir_a = w.tmp.path().join("backups-a");
    let dir_b = w.tmp.path().join("backups-b");
    w.ok(
        "",
        "set_settings",
        json!({"backup_dir": dir_a.to_str().unwrap()}),
    );
    w.ok(
        "1",
        "set_settings",
        json!({"backup_dir": dir_b.to_str().unwrap()}),
    );
    assert_eq!(
        w.ok("", "get_settings", json!({}))["backup_dir"],
        dir_a.to_str().unwrap()
    );
    assert_eq!(
        w.ok("1", "get_settings", json!({}))["backup_dir"],
        dir_b.to_str().unwrap()
    );

    // a snapshot of each graph goes to its own folder
    w.ok("", "backup_now", json!({}));
    w.ok("1", "backup_now", json!({}));
    let files = |d: &Path| std::fs::read_dir(d).unwrap().flatten().count();
    assert_eq!(files(&dir_a), 1);
    assert_eq!(files(&dir_b), 1);

    // encryption is set up for the first graph only
    w.ok(
        "",
        "backup_encryption_setup",
        json!({"password": "correct horse"}),
    );
    w.ok("", "backup_encryption_confirm", json!({}));
    let a = w.ok("", "backup_encryption_status", json!({}));
    let b = w.ok("1", "backup_encryption_status", json!({}));
    assert_eq!(
        (a["configured"].clone(), a["enabled"].clone()),
        (json!(true), json!(true))
    );
    assert_eq!(
        (b["configured"].clone(), b["enabled"].clone()),
        (json!(false), json!(false))
    );
    assert_eq!(w.ok("", "get_settings", json!({}))["backup_encrypt"], true);
    assert_eq!(
        w.ok("1", "get_settings", json!({}))["backup_encrypt"],
        false
    );
    // the next snapshot of the first graph is sealed, the second one's is plain
    w.ok("", "backup_now", json!({}));
    let sealed = |d: &Path| {
        std::fs::read_dir(d)
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().ends_with(".zip.enc"))
            .count()
    };
    assert_eq!(sealed(&dir_a), 1);
    assert_eq!(sealed(&dir_b), 0);
    // unlocking one graph's vault does not unlock another's
    w.ok(
        "1",
        "backup_encryption_setup",
        json!({"password": "another horse"}),
    );
    w.ok("1", "backup_encryption_confirm", json!({}));
    w.ok("", "backup_lock", json!({}));
    assert_eq!(
        w.ok("1", "backup_encryption_status", json!({}))["unlocked"],
        true
    );
    assert_eq!(
        w.ok("", "backup_encryption_status", json!({}))["unlocked"],
        false
    );
    assert!(
        w.call("", "backup_unlock", json!({"secret": "another horse"}))
            .is_err()
    );
}
