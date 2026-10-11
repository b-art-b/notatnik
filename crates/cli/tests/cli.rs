//! CLI tests: each command against a copy of `fixtures/graphs/basic` (snapshots), exit codes, help.
use std::path::Path;

use assert_cmd::Command;

struct Env {
    _tmp: tempfile::TempDir,
    graph: std::path::PathBuf,
    cfg: std::path::PathBuf,
    data: std::path::PathBuf,
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

fn env(fixture: &str) -> Env {
    let tmp = tempfile::tempdir().unwrap();
    let graph = tmp.path().join("graph");
    copy_dir(
        &Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/graphs")
            .join(fixture),
        &graph,
    );
    let _ = std::fs::remove_file(graph.join("README.md"));
    let _ = std::fs::remove_file(graph.join("expected-counts.json"));
    Env {
        cfg: tmp.path().join("cfg"),
        data: tmp.path().join("data"),
        graph,
        _tmp: tmp,
    }
}

impl Env {
    fn cmd(&self, args: &[&str]) -> Command {
        let mut c = Command::cargo_bin("notatnik").unwrap();
        c.env("XDG_CONFIG_HOME", &self.cfg)
            .env("XDG_DATA_HOME", &self.data)
            .args(args);
        c
    }
    fn run(&self, args: &[&str]) -> (String, String, i32) {
        let out = self.cmd(args).output().unwrap();
        (
            String::from_utf8_lossy(&out.stdout).into_owned(),
            String::from_utf8_lossy(&out.stderr).into_owned(),
            out.status.code().unwrap_or(-1),
        )
    }
    fn g(&self) -> &str {
        self.graph.to_str().unwrap()
    }
}

fn snap(name: &str, text: &str) {
    insta::with_settings!({ snapshot_path => "snapshots", prepend_module_to_snapshot => false }, {
        insta::assert_snapshot!(name, text);
    });
}

#[test]
fn index_search_query_backlinks_page() {
    let e = env("basic");
    let (out, _, code) = e.run(&["index", e.g()]);
    assert_eq!(code, 0);
    snap("index", &out);
    let (out, _, _) = e.run(&["index", e.g(), "--json"]);
    snap("index_json", &out);
    let (out, _, _) = e.run(&["index", e.g(), "--rebuild"]);
    snap("index_rebuild", &out);

    let (out, _, code) = e.run(&["search", e.g(), "zolw"]);
    assert_eq!(code, 0);
    snap("search", &out);
    let (out, _, _) = e.run(&["search", e.g(), "collect", "--json"]);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert!(v.as_array().unwrap().len() >= 2);

    let (out, _, _) = e.run(&[
        "query",
        e.g(),
        "task: todo\nsort: deadline asc",
        "--today",
        "2026-10-03",
    ]);
    snap("query", &out);
    let (out, _, _) = e.run(&[
        "query",
        e.g(),
        "task: any\nsort: deadline asc",
        "--today",
        "2026-10-03",
        "--json",
    ]);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["rows"].as_array().unwrap().len(), 7);
    std::fs::write(e.graph.join("q.query"), "tag: health\n").unwrap();
    let (out, _, _) = e.run(&[
        "query",
        e.g(),
        &format!("@{}", e.graph.join("q.query").display()),
    ]);
    snap("query_file", &out);

    let (out, _, _) = e.run(&["backlinks", e.g(), "people/anna"]);
    snap("backlinks", &out);
    let (out, _, _) = e.run(&["page", e.g(), "work/projects/alpha"]);
    snap("page", &out);
    let (out, _, _) = e.run(&["page", e.g(), "work/projects/alpha", "--json"]);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["frontmatter"]["title"], "Project Alpha");
}

#[test]
fn check_exit_codes_and_rename() {
    let e = env("basic");
    let (out, _, code) = e.run(&["check", e.g()]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("0 issues"));

    let (out, _, code) = e.run(&["rename", e.g(), "people/anna", "team/anna", "--dry-run"]);
    assert_eq!(code, 0);
    snap("rename_dry_run", &out);
    assert!(
        e.graph.join("pages/people/anna.md").exists(),
        "dry run changes nothing"
    );
    let (out, _, code) = e.run(&["rename", e.g(), "people/anna", "team/anna"]);
    assert_eq!(code, 0, "{out}");
    assert!(e.graph.join("pages/team/anna.md").exists());
    let (out, _, code) = e.run(&["check", e.g()]);
    assert_eq!(code, 0, "{out}");

    // a problem makes check exit with 2
    std::fs::write(
        e.graph.join("pages/broken.md"),
        "---\ntitle: [oops\n---\n- x\n",
    )
    .unwrap();
    let (out, _, code) = e.run(&["check", e.g()]);
    assert_eq!(code, 2);
    assert!(out.contains("invalid-frontmatter"), "{out}");
    let (out, _, code) = e.run(&["check", e.g(), "--json"]);
    assert_eq!(code, 2);
    assert!(serde_json::from_str::<serde_json::Value>(&out).is_ok());
}

#[test]
fn errors_exit_with_one_and_a_clear_message() {
    let e = env("basic");
    let (_, err, code) = e.run(&["query", e.g(), "tags: x"]);
    assert_eq!(code, 1);
    assert!(
        err.contains("unknown key `tags`; did you mean `tag`?"),
        "{err}"
    );
    let (_, err, code) = e.run(&["page", e.g(), "no/such/page"]);
    assert_eq!(code, 1);
    assert!(err.contains("no page"), "{err}");
    let (_, err, code) = e.run(&["index", "/definitely/not/here"]);
    assert_eq!(code, 1);
    assert!(err.contains("not found"), "{err}");
    let (_, err, code) = e.run(&["rename", e.g(), "work", "work/projects/x"]);
    assert_eq!(code, 1);
    assert!(err.contains("into itself"), "{err}");
}

#[test]
fn new_graph_creates_a_usable_graph() {
    let e = env("basic");
    let path = e._tmp.path().join("fresh");
    let (out, _, code) = e.run(&["new-graph", path.to_str().unwrap()]);
    assert_eq!(code, 0);
    assert!(out.starts_with("created graph g"));
    assert!(path.join(".notatnik/config.toml").exists() && path.join("pages").is_dir());
    std::fs::write(path.join("pages/hello.md"), "- hello [[world]]\n").unwrap();
    let (out, _, _) = e.run(&["backlinks", path.to_str().unwrap(), "world"]);
    assert!(out.contains("hello"));
}

#[test]
fn help_is_complete() {
    let e = env("basic");
    let (out, _, code) = e.run(&["--help"]);
    assert_eq!(code, 0);
    for cmd in [
        "new-graph",
        "index",
        "search",
        "query",
        "backlinks",
        "page",
        "check",
        "rename",
        "watch",
    ] {
        assert!(out.contains(cmd), "--help is missing {cmd}");
        let (h, _, code) = e.run(&[cmd, "--help"]);
        assert_eq!(code, 0);
        assert!(h.lines().count() > 3, "{cmd} --help is too short");
    }
    assert!(!out.contains("gen-graph"), "dev tools are hidden");
    snap("help", &out);
    let (out, _, _) = e.run(&["query", "--help"]);
    snap("help_query", &out);
}

#[test]
fn demo_script_from_the_phase_doc() {
    let e = env("basic");
    let (_, _, code) = e.run(&["search", e.g(), "zolw"]);
    assert_eq!(code, 0);
    let (_, _, code) = e.run(&["rename", e.g(), "people/anna", "team/anna", "--dry-run"]);
    assert_eq!(code, 0);
    let (_, _, code) = e.run(&["rename", e.g(), "people/anna", "team/anna"]);
    assert_eq!(code, 0);
    let (out, _, code) = e.run(&["check", e.g()]);
    assert_eq!((code, out.contains("0 issues")), (0, true));
}

#[test]
fn gen_graph_is_deterministic() {
    let e = env("basic");
    let a = e._tmp.path().join("a");
    let b = e._tmp.path().join("b");
    for d in [&a, &b] {
        let (_, _, code) = e.run(&[
            "dev",
            "gen-graph",
            "--seed",
            "7",
            "--pages",
            "40",
            "--journal-days",
            "10",
            "--out",
            d.to_str().unwrap(),
        ]);
        assert_eq!(code, 0);
    }
    fn tree(dir: &Path) -> Vec<(String, Vec<u8>)> {
        let mut out = Vec::new();
        fn walk(d: &Path, root: &Path, out: &mut Vec<(String, Vec<u8>)>) {
            let mut es: Vec<_> = std::fs::read_dir(d)
                .unwrap()
                .map(|e| e.unwrap().path())
                .collect();
            es.sort();
            for p in es {
                if p.is_dir() {
                    walk(&p, root, out);
                } else {
                    out.push((
                        p.strip_prefix(root).unwrap().to_string_lossy().into_owned(),
                        std::fs::read(&p).unwrap(),
                    ));
                }
            }
        }
        walk(dir, dir, &mut out);
        out
    }
    assert_eq!(tree(&a), tree(&b));
    assert_eq!(tree(&a).len(), 50);
    let (out, _, code) = e.run(&["check", a.to_str().unwrap()]);
    assert_eq!(code, 0, "{out}");
}
