//! Graph service integration tests on real temp directories (P1.30–P1.36, P1.9).
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use notatnik_graph::{Dirs, Error, Graph};

fn fixtures() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/graphs")
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

struct Env {
    _tmp: tempfile::TempDir,
    graph_dir: std::path::PathBuf,
    dirs: Dirs,
}

fn env_with(fixture: Option<&str>) -> Env {
    let tmp = tempfile::tempdir().unwrap();
    let graph_dir = tmp.path().join("graph");
    match fixture {
        Some(f) => copy_dir(&fixtures().join(f), &graph_dir),
        None => std::fs::create_dir_all(&graph_dir).unwrap(),
    }
    let dirs = Dirs::under(&tmp.path().join("device"));
    Env {
        _tmp: tmp,
        graph_dir,
        dirs,
    }
}

fn open(e: &Env) -> Graph {
    Graph::open(&e.graph_dir, &e.dirs).unwrap().0
}

fn names(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    v.sort();
    v
}

#[test]
fn create_then_open() {
    let e = env_with(None);
    let (g, rep) = Graph::create(&e.graph_dir, &e.dirs).unwrap();
    assert!(rep.warnings.is_empty());
    let id = g.id();
    assert!(id.starts_with('g') && id.len() == 7);
    assert_eq!(names(&e.graph_dir), [".notatnik", "journals", "pages"]);
    let cfg = std::fs::read_to_string(e.graph_dir.join(".notatnik/config.toml")).unwrap();
    assert!(cfg.contains(&id));
    drop(g);
    let (g2, _) = Graph::open(&e.graph_dir, &e.dirs).unwrap();
    assert_eq!(g2.id(), id, "the id is stable across opens");
    // the cache lives outside the graph, per device
    assert!(e.dirs.graph_data(&id).join("index.sqlite").exists());
    let settings = notatnik_graph::DeviceSettings::load(&e.dirs).unwrap();
    assert_eq!(settings.graphs.len(), 1);
    assert_eq!(settings.graphs[0].id, id);
}

#[test]
fn opening_a_plain_folder_of_markdown_files() {
    let e = env_with(None);
    std::fs::create_dir_all(e.graph_dir.join("pages/notes")).unwrap();
    std::fs::write(
        e.graph_dir.join("pages/notes/idea.md"),
        "- an idea with [[other]]\n",
    )
    .unwrap();
    std::fs::write(e.graph_dir.join("pages/other.md"), "- hello\n").unwrap();
    std::fs::write(e.graph_dir.join("README.txt"), "not a page").unwrap();
    let before = names(&e.graph_dir);
    let (g, rep) = Graph::open(&e.graph_dir, &e.dirs).unwrap();
    assert_eq!(rep.reconcile.reindexed, 2);
    let after = names(&e.graph_dir);
    assert_eq!(
        after
            .iter()
            .filter(|n| !before.contains(n))
            .collect::<Vec<_>>(),
        [".notatnik"],
        "only .notatnik is added"
    );
    assert_eq!(g.with_index(|i| i.counts().unwrap().pages), 2);
    assert_eq!(g.with_index(|i| i.backlinks("other").unwrap().len()), 1);
}

#[test]
fn config_warnings_for_unknown_keys() {
    let e = env_with(None);
    std::fs::create_dir_all(e.graph_dir.join(".notatnik")).unwrap();
    std::fs::write(
        e.graph_dir.join(".notatnik/config.toml"),
        "[graph]\nid = \"gabc123\"\nfuture = 1\n[mystery]\nx = 1\n",
    )
    .unwrap();
    let (g, rep) = Graph::open(&e.graph_dir, &e.dirs).unwrap();
    assert_eq!(g.id(), "gabc123");
    assert_eq!(rep.warnings.len(), 2, "{:?}", rep.warnings);
}

#[test]
fn declared_property_types_reach_the_parser() {
    let e = env_with(None);
    std::fs::create_dir_all(e.graph_dir.join(".notatnik")).unwrap();
    std::fs::write(
        e.graph_dir.join(".notatnik/config.toml"),
        "[graph]\nid = \"g1\"\n[properties]\ntypes = { zip = \"text\" }\n",
    )
    .unwrap();
    std::fs::create_dir_all(e.graph_dir.join("pages")).unwrap();
    std::fs::write(e.graph_dir.join("pages/p.md"), "- a\n  zip:: 2026-10-05\n").unwrap();
    let g = open(&e);
    let rows = g.with_index(|i| {
        let q = notatnik_core::parse_query("prop.zip: 2026-10-05").unwrap();
        i.run_query(
            &q,
            &notatnik_index::QueryContext {
                today: chrono::NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
                week_start: notatnik_core::WeekStart::Monday,
            },
        )
        .unwrap()
        .rows
    });
    assert_eq!(rows.len(), 1);
}

#[test]
fn guarded_write_detects_external_change() {
    let e = env_with(Some("basic"));
    let g = open(&e);
    let rel = "pages/inbox.md";
    let file = g.read_page(rel).unwrap();
    // someone else writes in between
    std::fs::write(e.graph_dir.join(rel), "- external content\n").unwrap();
    let err = g
        .write_page(rel, "- our content\n", Some(&file.hash))
        .unwrap_err();
    assert!(matches!(err, Error::Conflict { .. }), "{err}");
    assert_eq!(
        std::fs::read_to_string(e.graph_dir.join(rel)).unwrap(),
        "- external content\n"
    );
    // with the right hash it works, and the index is updated immediately
    let cur = g.read_page(rel).unwrap();
    g.write_page(rel, "- our content\n", Some(&cur.hash))
        .unwrap();
    assert_eq!(
        std::fs::read_to_string(e.graph_dir.join(rel)).unwrap(),
        "- our content\n"
    );
    assert!(g.with_index(|i| i.search("our content", 5).unwrap().len()) >= 1);
    // creating requires that the file does not exist
    assert!(matches!(
        g.write_page(rel, "x", None),
        Err(Error::Conflict { .. })
    ));
    g.write_page("pages/new/page.md", "- new\n", None).unwrap();
    assert!(e.graph_dir.join("pages/new/page.md").exists());
    // path safety
    assert!(matches!(
        g.write_page("../escape.md", "x", None),
        Err(Error::InvalidPath(_))
    ));
    assert!(matches!(
        g.write_page("assets/x.md", "x", None),
        Err(Error::InvalidPath(_))
    ));
}

#[test]
fn leftover_temp_files_are_removed_on_open() {
    let e = env_with(Some("basic"));
    drop(open(&e));
    let tmp = e.graph_dir.join(".notatnik/tmp");
    std::fs::write(tmp.join("w123-0"), "half written").unwrap();
    let original = std::fs::read(e.graph_dir.join("pages/inbox.md")).unwrap();
    let _g = open(&e);
    assert!(names(&tmp).is_empty());
    assert_eq!(
        std::fs::read(e.graph_dir.join("pages/inbox.md")).unwrap(),
        original
    );
}

#[test]
fn readers_never_see_a_partial_file() {
    let e = env_with(Some("basic"));
    let g = open(&e);
    let rel = "pages/inbox.md";
    let path = e.graph_dir.join(rel);
    let a: String = (0..400)
        .map(|i| format!("- line {i} aaaaaaaaaaaaaaaaaaaaaaaaaaaa\n"))
        .collect();
    let b: String = (0..400)
        .map(|i| format!("- line {i} bbbbbbbbbbbbbbbbbbbbbbbbbbbb\n"))
        .collect();
    let mut hash = g.read_page(rel).unwrap().hash;
    hash = g.write_page(rel, &a, Some(&hash)).unwrap().hash;
    let stop = Arc::new(AtomicBool::new(false));
    let bad = Arc::new(AtomicBool::new(false));
    let reader = {
        let (stop, bad, path, a, b) = (
            stop.clone(),
            bad.clone(),
            path.clone(),
            a.clone(),
            b.clone(),
        );
        std::thread::spawn(move || {
            while !stop.load(Ordering::Relaxed) {
                if let Ok(t) = std::fs::read_to_string(&path) {
                    if t != a && t != b {
                        bad.store(true, Ordering::Relaxed);
                    }
                }
            }
        })
    };
    for i in 0..1000 {
        let text = if i % 2 == 0 { &b } else { &a };
        hash = g.write_page(rel, text, Some(&hash)).unwrap().hash;
    }
    stop.store(true, Ordering::Relaxed);
    reader.join().unwrap();
    assert!(
        !bad.load(Ordering::Relaxed),
        "a reader observed a truncated or mixed file"
    );
}

fn wait_event(w: &notatnik_graph::Watcher, secs: u64) -> Option<notatnik_graph::GraphChanged> {
    w.events().recv_timeout(Duration::from_secs(secs)).ok()
}

#[test]
fn watcher_picks_up_external_writes_and_ignores_own() {
    let e = env_with(Some("basic"));
    let g = open(&e);
    let w = g.watch(Duration::from_millis(200)).unwrap();
    std::thread::sleep(Duration::from_millis(200));

    // external change
    let t = Instant::now();
    std::fs::write(
        e.graph_dir.join("pages/inbox.md"),
        "- brand new text from outside\n",
    )
    .unwrap();
    let ev = wait_event(&w, 5).expect("GraphChanged for an external write");
    assert!(
        t.elapsed() < Duration::from_secs(1),
        "took {:?}",
        t.elapsed()
    );
    assert_eq!(ev.pages, ["inbox"]);
    assert_eq!(g.with_index(|i| i.search("outside", 5).unwrap().len()), 1);

    // our own write: no event
    let cur = g.read_page("pages/inbox.md").unwrap();
    g.write_page("pages/inbox.md", "- written by us\n", Some(&cur.hash))
        .unwrap();
    assert!(
        wait_event(&w, 1).is_none(),
        "own writes must not produce events"
    );
    assert_eq!(g.watcher_reindex_count(), 1);

    // a new file and a deletion
    std::fs::write(e.graph_dir.join("pages/fresh.md"), "- fresh page\n").unwrap();
    let ev = wait_event(&w, 5).unwrap();
    assert_eq!(ev.pages, ["fresh"]);
    std::fs::remove_file(e.graph_dir.join("pages/fresh.md")).unwrap();
    let ev = wait_event(&w, 5).unwrap();
    assert_eq!(ev.pages, ["fresh"]);
    assert!(g.with_index(|i| i.page_info("fresh").unwrap()).is_none());
}

#[test]
fn rapid_writes_are_coalesced() {
    let e = env_with(Some("basic"));
    let g = open(&e);
    let w = g.watch(Duration::from_millis(200)).unwrap();
    std::thread::sleep(Duration::from_millis(200));
    for i in 0..50 {
        std::fs::write(
            e.graph_dir.join("pages/inbox.md"),
            format!("- version {i}\n"),
        )
        .unwrap();
        std::thread::sleep(Duration::from_millis(5));
    }
    let mut events = 0;
    while wait_event(&w, 1).is_some() {
        events += 1;
    }
    assert!(events >= 1);
    assert!(
        g.watcher_reindex_count() <= 3,
        "reindexed {} times",
        g.watcher_reindex_count()
    );
    assert_eq!(
        g.with_index(|i| i
            .search("version", 5)
            .unwrap()
            .iter()
            .filter(|h| h.text == "version 49")
            .count()),
        1
    );
}

#[test]
fn temp_file_plus_rename_is_one_update_without_flicker() {
    let e = env_with(Some("basic"));
    let g = open(&e);
    let w = g.watch(Duration::from_millis(200)).unwrap();
    std::thread::sleep(Duration::from_millis(200));
    let target = e.graph_dir.join("pages/inbox.md");
    let tmp = e.graph_dir.join("pages/.inbox.md.tmp123");
    std::fs::write(&tmp, "- saved via temp file and rename\n").unwrap();
    std::fs::rename(&tmp, &target).unwrap();
    let ev = wait_event(&w, 5).unwrap();
    assert_eq!(ev.pages, ["inbox"]);
    assert!(wait_event(&w, 1).is_none(), "a single update");
    assert_eq!(g.watcher_reindex_count(), 1);
    assert!(g.with_index(|i| i.page_info("inbox").unwrap()).is_some());
}

#[test]
fn startup_reconcile_after_offline_changes() {
    let e = env_with(Some("basic"));
    drop(open(&e));
    let p = |s: &str| e.graph_dir.join(s);
    std::fs::write(p("pages/inbox.md"), "- changed while closed\n").unwrap();
    std::fs::remove_file(p("journals/2026/2026-10-02.md")).unwrap();
    std::fs::write(p("pages/added.md"), "- added while closed\n").unwrap();
    let (g, rep) = Graph::open(&e.graph_dir, &e.dirs).unwrap();
    assert_eq!(
        (
            rep.reconcile.reindexed,
            rep.reconcile.added,
            rep.reconcile.deleted
        ),
        (2, 1, 1)
    );
    assert_eq!(g.with_index(|i| i.search("closed", 5).unwrap().len()), 2);
}

fn graph_files_snapshot(dir: &Path) -> std::collections::BTreeMap<String, Vec<u8>> {
    let mut out = std::collections::BTreeMap::new();
    fn walk(dir: &Path, root: &Path, out: &mut std::collections::BTreeMap<String, Vec<u8>>) {
        for e in std::fs::read_dir(dir).unwrap() {
            let e = e.unwrap();
            if e.file_name().to_string_lossy().starts_with('.') {
                continue;
            }
            if e.file_type().unwrap().is_dir() {
                walk(&e.path(), root, out);
            } else {
                out.insert(
                    e.path()
                        .strip_prefix(root)
                        .unwrap()
                        .to_string_lossy()
                        .into_owned(),
                    std::fs::read(e.path()).unwrap(),
                );
            }
        }
    }
    walk(dir, dir, &mut out);
    out
}

#[test]
fn rename_rewrites_references_and_moves_the_file() {
    let e = env_with(Some("basic"));
    let g = open(&e);
    let before = graph_files_snapshot(&e.graph_dir);
    let plan = g.plan_rename("people/anna", "team/anna").unwrap();
    assert_eq!(
        plan.file_moves,
        [(
            "pages/people/anna.md".to_string(),
            "pages/team/anna.md".to_string()
        )]
    );
    let edited: Vec<&str> = plan.edits.iter().map(|e| e.path.as_str()).collect();
    assert_eq!(
        edited,
        [
            "journals/2026/2026-10-01.md",
            "pages/meetings/2026-09-01 alpha kickoff.md",
            "pages/work/projects/alpha.md"
        ]
    );
    // dry run changed nothing
    assert_eq!(graph_files_snapshot(&e.graph_dir), before);

    let report = g.apply_rename(&plan).unwrap();
    assert_eq!(report.rewritten.len(), 3);
    let after = graph_files_snapshot(&e.graph_dir);
    for (path, content) in &before {
        if edited.contains(&path.as_str()) {
            assert_ne!(&after[path], content, "{path} should be rewritten");
        } else if path == "pages/people/anna.md" {
            assert_eq!(&after["pages/team/anna.md"], content, "moved unchanged");
        } else {
            assert_eq!(&after[path], content, "{path} must be byte-identical");
        }
    }
    assert!(
        !e.graph_dir.join("pages/people").exists(),
        "empty folder removed"
    );
    let alpha = String::from_utf8(after["pages/work/projects/alpha.md"].clone()).unwrap();
    assert!(alpha.contains("Owner:: [[team/anna]]") && !alpha.contains("people/anna"));
    let rep = g.check().unwrap();
    assert!(!rep.has_problems(), "{rep:?}");
    assert!(
        rep.unresolved_links
            .iter()
            .all(|(_, t)| !t.contains("people/anna"))
    );
    assert_eq!(g.with_index(|i| i.backlinks("team/anna").unwrap().len()), 3);
    assert!(
        g.with_index(|i| i.page_info("people/anna").unwrap())
            .is_none(),
        "old name is gone"
    );
}

#[test]
fn rename_rewrites_labels_tags_and_keeps_spelling_variants_working() {
    let e = env_with(None);
    std::fs::create_dir_all(e.graph_dir.join("pages")).unwrap();
    std::fs::write(e.graph_dir.join("pages/old.md"), "- the page\n").unwrap();
    std::fs::write(
        e.graph_dir.join("pages/refs.md"),
        "- [[old]] and [[ OLD ]] and [[old|shown]] and ![[old]]\n- #old and #[[old]] and (#old) and #older\n- `[[old]]` code and [[older]]\n",
    )
    .unwrap();
    let g = open(&e);
    let plan = g.plan_rename("old", "brand new").unwrap();
    g.apply_rename(&plan).unwrap();
    let t = std::fs::read_to_string(e.graph_dir.join("pages/refs.md")).unwrap();
    assert_eq!(
        t,
        "- [[brand new]] and [[brand new]] and [[brand new|shown]] and ![[brand new]]\n- #[[brand new]] and #[[brand new]] and (#[[brand new]]) and #older\n- `[[old]]` code and [[older]]\n"
    );
    assert!(e.graph_dir.join("pages/brand new.md").exists());
    // a rename to a simple name keeps plain tag syntax
    let plan = g.plan_rename("brand new", "simple").unwrap();
    g.apply_rename(&plan).unwrap();
    let t = std::fs::read_to_string(e.graph_dir.join("pages/refs.md")).unwrap();
    assert!(t.contains("#simple and") || t.contains("#[[simple]]"));
    assert!(t.contains("[[simple|shown]]"));
}

#[test]
fn rename_is_refused_in_unsafe_situations() {
    let e = env_with(Some("namespaces"));
    let g = open(&e);
    let before = graph_files_snapshot(&e.graph_dir);
    let err = g
        .plan_rename("work", "work/projects/x")
        .unwrap_err()
        .to_string();
    assert!(err.contains("into itself"), "{err}");
    // a subpage destination that exists blocks the whole move
    let err = g
        .plan_rename("work/projects", "workshop")
        .and_then(|_| g.plan_rename("work", "workshop"))
        .unwrap_err()
        .to_string();
    assert!(err.contains("already exists"), "{err}");
    let err = g
        .plan_rename("workshop", "work/projects/alpha")
        .unwrap_err()
        .to_string();
    assert!(err.contains("already exists"), "{err}");
    assert!(g.plan_rename("2026-10-03", "x").is_err());
    assert!(g.plan_rename("workshop", "workshop").is_err());
    assert!(matches!(
        g.plan_rename("nope/nothing", "x"),
        Err(Error::NotFound(_))
    ));
    assert_eq!(graph_files_snapshot(&e.graph_dir), before);
}

#[test]
fn rename_stops_on_conflict_and_reports_progress() {
    let e = env_with(Some("basic"));
    let g = open(&e);
    let plan = g.plan_rename("people/anna", "team/anna").unwrap();
    // an external edit lands in the second file after planning
    std::fs::write(
        e.graph_dir.join(&plan.edits[1].path),
        "- someone else's edit [[people/anna]]\n",
    )
    .unwrap();
    let err = g.apply_rename(&plan).unwrap_err();
    match err {
        Error::PartialRename { done, failed } => {
            assert_eq!(done, [plan.edits[0].path.clone()]);
            assert_eq!(failed, plan.edits[1].path);
        }
        other => panic!("{other}"),
    }
    assert!(
        e.graph_dir.join("pages/people/anna.md").exists(),
        "nothing was moved"
    );
    assert_eq!(
        std::fs::read_to_string(e.graph_dir.join(&plan.edits[1].path)).unwrap(),
        "- someone else's edit [[people/anna]]\n"
    );
}

#[test]
fn conflict_copies_are_listed_and_not_indexed() {
    let e = env_with(Some("basic"));
    std::fs::write(
        e.graph_dir
            .join("pages/work/alpha (conflicted copy 2026-10-03 120000).md"),
        "- zebra conflict\n",
    )
    .unwrap();
    let g = open(&e);
    let rep = g.check().unwrap();
    assert_eq!(
        rep.conflicts,
        ["pages/work/alpha (conflicted copy 2026-10-03 120000).md"]
    );
    assert!(rep.has_problems());
    assert!(g.with_index(|i| i.search("zebra", 5).unwrap()).is_empty());
}

#[test]
fn history_snapshots_follow_writes_and_external_changes() {
    let e = env_with(Some("basic"));
    let g = open(&e);
    let rel = "pages/inbox.md";
    let on_disk = |g: &Graph| g.read_page(rel).unwrap().text;
    assert_eq!(
        g.history_snapshot(rel).unwrap(),
        on_disk(&g),
        "snapshot after the initial index"
    );
    let h = g.read_page(rel).unwrap().hash;
    g.write_page(rel, "- after our write\n", Some(&h)).unwrap();
    assert_eq!(g.history_snapshot(rel).unwrap(), "- after our write\n");
    // external change picked up by the watcher
    let w = g.watch(Duration::from_millis(100)).unwrap();
    std::thread::sleep(Duration::from_millis(200));
    std::fs::write(e.graph_dir.join(rel), "- external\n").unwrap();
    wait_event(&w, 5).unwrap();
    assert_eq!(g.history_snapshot(rel).unwrap(), "- external\n");
    // no unbounded growth
    let count = g.history_count();
    let size = g.history_size();
    let mut h = g.read_page(rel).unwrap().hash;
    for i in 0..100 {
        h = g
            .write_page(rel, &format!("- write {i}\n"), Some(&h))
            .unwrap()
            .hash;
    }
    assert_eq!(g.history_count(), count, "still one snapshot per file");
    assert!(
        g.history_size() <= size + 100,
        "history size stays about the size of the graph"
    );
    // deleting a page removes its snapshot after reconcile
    std::fs::remove_file(e.graph_dir.join("pages/people/anna.md")).unwrap();
    g.reconcile().unwrap();
    assert!(g.history_snapshot("pages/people/anna.md").is_none());
}

#[test]
fn duplicate_block_id_gets_a_new_id_and_is_reported() {
    let e = env_with(Some("refs"));
    let g = open(&e);
    // copy a block with its id into another file, in a text editor
    std::fs::write(
        e.graph_dir.join("pages/zcopy.md"),
        "- Copy of the top block ^aaa111\n",
    )
    .unwrap();
    let w = g.watch(Duration::from_millis(100)).unwrap();
    std::thread::sleep(Duration::from_millis(200));
    std::fs::write(
        e.graph_dir.join("pages/zcopy.md"),
        "- Copy of the top block ^aaa111\n- another line\n",
    )
    .unwrap();
    wait_event(&w, 5).unwrap();
    std::thread::sleep(Duration::from_millis(500));
    let text = std::fs::read_to_string(e.graph_dir.join("pages/zcopy.md")).unwrap();
    assert!(!text.contains("^aaa111"), "the copy got a new id: {text}");
    assert!(text.contains(" ^") && text.contains("another line"));
    // the original keeps the id and the reference still resolves to it
    let (page, _, _) = g.with_index(|i| i.block_by_uid("aaa111").unwrap()).unwrap();
    assert_eq!(page, "source");
    assert_eq!(
        g.with_index(|i| i.dangling_block_refs().unwrap().len()),
        1,
        "only the intentionally dangling ((zzz999))"
    );
    // the health check reports the re-id
    let rep = g.check().unwrap();
    assert_eq!(rep.reids.len(), 1);
    assert_eq!(rep.reids[0].old, "aaa111");
    assert_eq!(rep.reids[0].path, "pages/zcopy.md");
}

#[test]
fn cold_open_with_duplicate_ids_keeps_the_first_file_by_path() {
    let e = env_with(Some("refs"));
    std::fs::write(
        e.graph_dir.join("pages/aaa-first.md"),
        "- Earlier file ^aaa111\n",
    )
    .unwrap();
    let (g, rep) = Graph::open(&e.graph_dir, &e.dirs).unwrap();
    assert_eq!(rep.reids.len(), 1);
    assert_eq!(
        rep.reids[0].path, "pages/source.md",
        "source.md sorts after aaa-first.md and loses the id"
    );
    assert_eq!(
        g.with_index(|i| i.block_by_uid("aaa111").unwrap())
            .unwrap()
            .0,
        "aaa-first"
    );
}

#[test]
fn edit_page_applies_ops_with_guard() {
    use notatnik_core::{BlockOp, BlockRef};
    let e = env_with(Some("basic"));
    let g = open(&e);
    let rel = "pages/inbox.md";
    let f = g.read_page(rel).unwrap();
    let out = g
        .edit_page(
            rel,
            Some(&f.hash),
            &[BlockOp::SetTaskState {
                block: BlockRef::at(&[0, 0]),
                state: Some(notatnik_core::TaskState::Todo),
                done_at: None,
                completed_limit: None,
            }],
        )
        .unwrap();
    assert!(out.new_text.contains("- TODO Call the dentist"));
    assert_eq!(out.patches.len(), 1);
    // stale base hash
    let err = g
        .edit_page(
            rel,
            Some(&f.hash),
            &[BlockOp::Delete {
                block: BlockRef::at(&[0]),
            }],
        )
        .unwrap_err();
    assert!(matches!(err, Error::Conflict { .. }));
    // a rejected op writes nothing
    let cur = g.read_page(rel).unwrap();
    let err = g
        .edit_page(
            rel,
            Some(&cur.hash),
            &[BlockOp::Indent {
                block: BlockRef::at(&[0]),
            }],
        )
        .unwrap_err();
    assert!(matches!(err, Error::Edit(_)));
    assert_eq!(g.read_page(rel).unwrap().text, cur.text);
    // None creates the page from nothing; an existing file with None is a conflict
    let out = g
        .edit_page(
            "pages/created/by-edit.md",
            None,
            &[BlockOp::Insert {
                parent: vec![],
                index: 0,
                text: "first".into(),
            }],
        )
        .unwrap();
    assert_eq!(out.new_text, "- first\n");
    assert!(e.graph_dir.join("pages/created/by-edit.md").exists());
    assert!(matches!(
        g.edit_page("pages/created/by-edit.md", None, &[]),
        Err(Error::Conflict { .. })
    ));
}

#[test]
fn moving_a_namespace_moves_all_pages_and_rewrites_all_links() {
    let e = env_with(Some("namespaces"));
    let g = open(&e);
    let before = graph_files_snapshot(&e.graph_dir);
    // work/projects (alpha, beta) -> archive/2026/projects; `gamma` only exists as a link target
    let plan = g
        .plan_rename("work/projects", "archive/2026/projects")
        .unwrap();
    assert_eq!(
        plan.file_moves,
        [
            (
                "pages/work/projects/alpha.md".to_string(),
                "pages/archive/2026/projects/alpha.md".to_string()
            ),
            (
                "pages/work/projects/beta.md".to_string(),
                "pages/archive/2026/projects/beta.md".to_string()
            ),
        ]
    );
    let edited: Vec<&str> = plan.edits.iter().map(|e| e.path.as_str()).collect();
    assert_eq!(edited, ["pages/people/anna.md"]);
    assert_eq!(graph_files_snapshot(&e.graph_dir), before, "dry run");
    g.apply_rename(&plan).unwrap();
    let after = graph_files_snapshot(&e.graph_dir);
    let anna = String::from_utf8(after["pages/people/anna.md"].clone()).unwrap();
    assert!(anna.contains("[[archive/2026/projects/gamma]]") && anna.contains("[[x/y/z]]"));
    for (path, content) in &before {
        match path.as_str() {
            "pages/people/anna.md" => {}
            "pages/work/projects/alpha.md" => assert_eq!(
                &after["pages/archive/2026/projects/alpha.md"], content,
                "moved unchanged"
            ),
            "pages/work/projects/beta.md" => assert_eq!(
                &after["pages/archive/2026/projects/beta.md"], content,
                "moved unchanged"
            ),
            _ => assert_eq!(&after[path], content, "{path} must be byte-identical"),
        }
    }
    assert!(!e.graph_dir.join("pages/work/projects").exists());
    assert!(
        e.graph_dir.join("pages/work.md").exists(),
        "the parent stays"
    );
    let rep = g.check().unwrap();
    assert!(!rep.has_problems(), "{rep:?}");
    assert!(
        rep.unresolved_links
            .iter()
            .all(|(_, t)| !t.contains("work/projects")),
        "{rep:?}"
    );
    assert!(
        g.with_index(|i| i.page_info("archive/2026/projects/alpha").unwrap())
            .is_some()
    );
    assert!(
        g.with_index(|i| i.page_info("work/projects/alpha").unwrap())
            .is_none()
    );
}

#[test]
fn renaming_a_namespace_root_moves_its_own_page_and_children() {
    let e = env_with(Some("namespaces"));
    let g = open(&e);
    let plan = g.plan_rename("work", "jobs").unwrap();
    g.apply_rename(&plan).unwrap();
    for f in [
        "pages/jobs.md",
        "pages/jobs/projects/alpha.md",
        "pages/jobs/projects/beta.md",
    ] {
        assert!(e.graph_dir.join(f).exists(), "{f}");
    }
    assert!(!e.graph_dir.join("pages/work.md").exists());
    assert!(!e.graph_dir.join("pages/work").exists());
    let alpha = std::fs::read_to_string(e.graph_dir.join("pages/jobs/projects/alpha.md")).unwrap();
    assert!(alpha.contains("[[jobs]]"), "{alpha}");
    // `workshop` only shares a prefix: untouched
    assert!(e.graph_dir.join("pages/workshop.md").exists());
    assert!(e.graph_dir.join("pages/workshop/x.md").exists());
    assert!(!g.check().unwrap().has_problems());
}

#[test]
fn moving_a_virtual_namespace_moves_only_the_files_below_it() {
    let e = env_with(Some("namespaces"));
    let g = open(&e);
    // `a` and `a/b/...` have no files except the deepest page
    let plan = g.plan_rename("a", "z").unwrap();
    assert_eq!(
        plan.file_moves,
        [(
            "pages/a/b/c/d/deep.md".to_string(),
            "pages/z/b/c/d/deep.md".to_string()
        )]
    );
    g.apply_rename(&plan).unwrap();
    assert!(e.graph_dir.join("pages/z/b/c/d/deep.md").exists());
    assert!(!e.graph_dir.join("pages/a").exists());
}

#[test]
fn a_namespace_move_that_stops_on_a_conflict_can_be_reverted_byte_for_byte() {
    let e = env_with(Some("namespaces"));
    let g = open(&e);
    let before = graph_files_snapshot(&e.graph_dir);
    let plan = g.plan_rename("work", "jobs").unwrap();
    assert_eq!(plan.edits.len(), 2, "anna.md and alpha.md");
    // an external edit lands in the second file after planning
    let second = plan.edits[1].path.clone();
    std::fs::write(
        e.graph_dir.join(&second),
        "- someone else's edit, still links to [[work]]\n",
    )
    .unwrap();
    let err = g.apply_rename(&plan).unwrap_err();
    assert!(
        matches!(&err, Error::PartialRename { done, failed }
            if *done == [plan.edits[0].path.clone()] && *failed == second),
        "{err}"
    );
    let restored = g.revert_rename(&plan).unwrap();
    assert_eq!(restored, [plan.edits[0].path.clone()]);
    let mut expected = before.clone();
    expected.insert(
        second.clone(),
        "- someone else's edit, still links to [[work]]\n"
            .as_bytes()
            .to_vec(),
    );
    assert_eq!(
        graph_files_snapshot(&e.graph_dir),
        expected,
        "everything but the external edit is back"
    );
    // "finish": planning again picks up where it stopped and succeeds
    let plan2 = g.plan_rename("work", "jobs").unwrap();
    g.apply_rename(&plan2).unwrap();
    assert!(e.graph_dir.join("pages/jobs.md").exists());
    let alpha = std::fs::read_to_string(e.graph_dir.join("pages/jobs/projects/alpha.md")).unwrap();
    assert!(alpha.contains("[[jobs]]"), "{alpha}");
}

#[test]
fn reverting_a_complete_namespace_move_puts_everything_back() {
    let e = env_with(Some("namespaces"));
    let g = open(&e);
    let before = graph_files_snapshot(&e.graph_dir);
    let plan = g.plan_rename("work", "jobs").unwrap();
    g.apply_rename(&plan).unwrap();
    g.revert_rename(&plan).unwrap();
    assert_eq!(graph_files_snapshot(&e.graph_dir), before);
    assert!(
        g.with_index(|i| i.page_info("work/projects/alpha").unwrap())
            .is_some()
    );
}

#[test]
fn a_page_can_move_alone_to_a_name_below_itself_while_its_subpages_stay() {
    let e = env_with(Some("namespaces"));
    let g = open(&e);
    // with the subpages it is impossible, and the message says what to do
    let err = g
        .plan_rename("work", "work/archive/old")
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("into itself") && err.contains("move only this page"),
        "{err}"
    );
    // only the page: its file moves, links to it follow, nothing below it is touched
    let before = graph_files_snapshot(&e.graph_dir);
    let plan = g
        .plan_rename_with("work", "work/archive/old", false)
        .unwrap();
    assert_eq!(
        plan.file_moves,
        [(
            "pages/work.md".to_string(),
            "pages/work/archive/old.md".to_string()
        )]
    );
    g.apply_rename(&plan).unwrap();
    let after = graph_files_snapshot(&e.graph_dir);
    assert_eq!(after["pages/work/archive/old.md"], before["pages/work.md"]);
    assert!(!after.contains_key("pages/work.md"));
    for p in ["pages/work/projects/beta.md", "pages/workshop.md"] {
        assert_eq!(after[p], before[p], "{p} untouched");
    }
    // `alpha` linked to [[work]]: only that link was rewritten
    let alpha = String::from_utf8(after["pages/work/projects/alpha.md"].clone()).unwrap();
    assert!(alpha.contains("[[work/archive/old]]"), "{alpha}");
    // the subpages are still there, still below `work`
    assert!(
        g.with_index(|i| i.page_info("work/projects/alpha").unwrap())
            .is_some()
    );
    assert!(
        g.with_index(|i| i.page_info("work/archive/old").unwrap())
            .is_some()
    );
    assert!(!g.check().unwrap().has_problems());
    // moving only one page never rewrites links to its subpages
    let plan = g
        .plan_rename_with("work/archive/old", "elsewhere", false)
        .unwrap();
    assert!(
        plan.edits
            .iter()
            .all(|e| e.patches.iter().all(|p| !p.old_text.contains("projects"))),
        "{plan:?}"
    );
}

// ---- backups (BK-1..BK-5) -----------------------------------------------------------------

fn stamp(s: &str) -> chrono::NaiveDateTime {
    chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S").unwrap()
}

#[test]
fn a_snapshot_holds_the_graph_files_and_restores_byte_for_byte() {
    let e = env_with(Some("basic"));
    let g = open(&e);
    let dir = e.dirs.data.join("backups");
    let made = g
        .backup_now(&dir, stamp("2026-10-10 09:00:00"), false, None)
        .unwrap()
        .expect("first snapshot");
    assert!(made.name.starts_with(&format!("notatnik-{}-", g.id())));
    assert!(made.name.ends_with("-20261010-090000.zip"));
    // no temp files are left, and nothing from `.notatnik/tmp` is inside
    assert!(names(&dir).iter().all(|n| !n.ends_with(".partial")));
    let target = e.dirs.data.join("restored");
    let n = g.restore_backup_to(&dir, &made.name, &target).unwrap();
    assert!(n > 0);
    for f in walk_files(&e.graph_dir) {
        let rel = f.strip_prefix(&e.graph_dir).unwrap();
        if rel.starts_with(".notatnik/tmp") {
            continue;
        }
        assert_eq!(
            std::fs::read(&f).unwrap(),
            std::fs::read(target.join(rel)).unwrap(),
            "{}",
            rel.display()
        );
    }
    // a restored folder opens as a graph
    let (g2, _) = Graph::open(&target, &Dirs::under(&e.dirs.data.join("other"))).unwrap();
    assert_eq!(g2.id(), g.id());
    // a restore target must be empty
    assert!(g.restore_backup_to(&dir, &made.name, &target).is_err());
    // and outside the graph
    assert!(
        g.restore_backup_to(&dir, &made.name, &e.graph_dir.join("again"))
            .is_err()
    );
}

fn walk_files(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(dir).unwrap() {
        let e = e.unwrap();
        if e.file_type().unwrap().is_dir() {
            out.extend(walk_files(&e.path()));
        } else {
            out.push(e.path());
        }
    }
    out
}

#[test]
fn an_unchanged_graph_is_not_saved_again_and_a_daily_limit_applies() {
    let e = env_with(Some("basic"));
    let g = open(&e);
    let dir = e.dirs.data.join("backups");
    let day = chrono::Duration::hours(24);
    assert!(
        g.backup_now(&dir, stamp("2026-10-10 09:00:00"), false, Some(day))
            .unwrap()
            .is_some()
    );
    // unchanged: nothing, even a day later
    assert!(
        g.backup_now(&dir, stamp("2026-10-11 10:00:00"), false, Some(day))
            .unwrap()
            .is_none()
    );
    // changed, but the newest snapshot is younger than a day
    let page = g.read_page("pages/inbox.md").unwrap();
    g.write_page(
        "pages/inbox.md",
        &format!("{}- new\n", page.text),
        Some(&page.hash),
    )
    .unwrap();
    assert!(
        g.backup_now(&dir, stamp("2026-10-10 20:00:00"), false, Some(day))
            .unwrap()
            .is_none()
    );
    // changed and old enough
    assert!(
        g.backup_now(&dir, stamp("2026-10-11 10:00:00"), false, Some(day))
            .unwrap()
            .is_some()
    );
    assert_eq!(g.backups(&dir).len(), 2);
    // forced: always, even unchanged; a name taken in that second is not reused. Retention keeps one
    // snapshot per day, so the new one replaces the older one of the same day
    let a = g
        .backup_now(&dir, stamp("2026-10-11 10:00:00"), true, None)
        .unwrap()
        .unwrap();
    let all = g.backups(&dir);
    assert_eq!(all.len(), 2);
    assert_eq!(all[0].name, a.name);
    assert!(a.name.ends_with("-20261011-100001.zip"));
    // the folder must not be inside the graph
    assert!(
        g.backup_now(
            &e.graph_dir.join("bk"),
            stamp("2026-10-12 10:00:00"),
            true,
            None
        )
        .is_err()
    );
}

#[test]
fn one_page_comes_back_from_a_snapshot() {
    let e = env_with(Some("basic"));
    let g = open(&e);
    let dir = e.dirs.data.join("backups");
    let before = g.read_page("pages/inbox.md").unwrap();
    let made = g
        .backup_now(&dir, stamp("2026-10-10 09:00:00"), false, None)
        .unwrap()
        .unwrap();
    g.write_page("pages/inbox.md", "- ruined\n", Some(&before.hash))
        .unwrap();
    g.restore_page_from_backup(
        &dir,
        &made.name,
        "pages/inbox.md",
        stamp("2026-10-10 10:00:00"),
    )
    .unwrap();
    assert_eq!(g.read_page("pages/inbox.md").unwrap().text, before.text);
    // the ruined version is in a snapshot of its own
    let newest = &g.backups(&dir)[0];
    assert_ne!(newest.name, made.name);
    let copy = e.dirs.data.join("ruined");
    g.restore_backup_to(&dir, &newest.name, &copy).unwrap();
    assert_eq!(
        std::fs::read_to_string(copy.join("pages/inbox.md")).unwrap(),
        "- ruined\n"
    );
    let newest_name = newest.name.clone();
    // a page that is not in the snapshot, and names that are not snapshot names
    assert!(matches!(
        g.restore_page_from_backup(
            &dir,
            &newest_name,
            "pages/nothing-here.md",
            stamp("2026-10-10 10:00:00")
        ),
        Err(Error::NotFound(_))
    ));
    assert!(
        g.restore_page_from_backup(
            &dir,
            "../x.zip",
            "pages/inbox.md",
            stamp("2026-10-10 10:00:00")
        )
        .is_err()
    );
}
