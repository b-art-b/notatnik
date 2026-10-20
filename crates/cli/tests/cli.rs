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

/// EN-5: `decrypt` opens a sealed backup with the password or the recovery key, and refuses a wrong one without
/// leaving a file behind.
#[test]
fn decrypt_opens_a_sealed_file_with_the_password_or_the_recovery_key() {
    use notatnik_crypt::{KdfParams, Vault, seal};
    let tmp = tempfile::tempdir().unwrap();
    let (vault, _keys, recovery) =
        Vault::create("a long password", KdfParams::insecure_for_tests()).unwrap();
    let plain = b"PK-not-really-a-zip but some bytes".repeat(5000);
    let file = tmp.path().join("notatnik-gx-20261010-090000.zip.enc");
    let mut out = Vec::new();
    seal(&vault, b"c", &mut &plain[..], &mut out).unwrap();
    std::fs::write(&file, out).unwrap();
    let run = |secret: &str, extra: &[&str]| {
        Command::cargo_bin("notatnik")
            .unwrap()
            .env("NOTATNIK_PASSWORD", secret)
            .arg("decrypt")
            .arg(&file)
            .args(extra)
            .assert()
    };
    // wrong password: error, no output
    run("nope nope nope", &[]).failure().code(1);
    assert!(!tmp.path().join("notatnik-gx-20261010-090000.zip").exists());
    assert!(
        std::fs::read_dir(tmp.path())
            .unwrap()
            .flatten()
            .all(|e| !e.file_name().to_string_lossy().ends_with(".partial"))
    );
    // the password
    run("a long password", &[]).success();
    let dest = tmp.path().join("notatnik-gx-20261010-090000.zip");
    assert_eq!(std::fs::read(&dest).unwrap(), plain);
    // an existing output is kept unless --force
    run("a long password", &[]).failure().code(1);
    std::fs::write(&dest, "old").unwrap();
    run("a long password", &["--force"]).success();
    assert_eq!(std::fs::read(&dest).unwrap(), plain);
    // the recovery key, to another file
    let other = tmp.path().join("other.zip");
    run(&recovery.to_string(), &["-o", other.to_str().unwrap()]).success();
    assert_eq!(std::fs::read(&other).unwrap(), plain);
    // a file that is not sealed
    let junk = tmp.path().join("junk.enc");
    std::fs::write(&junk, b"hello").unwrap();
    Command::cargo_bin("notatnik")
        .unwrap()
        .env("NOTATNIK_PASSWORD", "x")
        .arg("decrypt")
        .arg(&junk)
        .assert()
        .failure()
        .code(1);
}

/// EN-20: `verify` reads the server copy back (decrypting what is stored encrypted), compares it with the graph
/// folder, changes nothing and exits 2 when something differs.
#[test]
fn verify_checks_the_server_copy_against_the_graph() {
    use notatnik_core::encrypted::EncryptedPaths;
    use notatnik_crypt::{KdfParams, Vault};
    use notatnik_sync::fakedav::FakeDav;
    use notatnik_sync::{Encryption, Options, WebDavRemote, sync};
    use std::sync::Arc;

    let tmp = tempfile::tempdir().unwrap();
    let graph = tmp.path().join("graph");
    for (rel, text) in [
        ("pages/open.md", "- open OPENMARKER\n"),
        ("pages/private/diary.md", "- SECRETMARKER\n"),
        (
            ".notatnik/config.toml",
            "[encryption]\npaths = [\"pages/private/\"]\n",
        ),
    ] {
        let p = graph.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }
    let (vault, keys, _) =
        Vault::create("graph password", KdfParams::insecure_for_tests()).unwrap();
    std::fs::write(graph.join(".notatnik/vault.json"), vault.to_json()).unwrap();

    // opening a folder gives it a graph ID in its config: do that before the first sync, as the app does
    Command::cargo_bin("notatnik")
        .unwrap()
        .env("XDG_CONFIG_HOME", tmp.path().join("cfg"))
        .env("XDG_DATA_HOME", tmp.path().join("data"))
        .args(["index"])
        .arg(&graph)
        .assert()
        .success();
    let dav = FakeDav::start();
    let url = dav.folder_url("Notes/g");
    let remote = WebDavRemote::new(&url, &dav.user, &dav.password).unwrap();
    let mut opts = Options::new(tmp.path().join("state.json"));
    opts.encryption = Some(Encryption {
        paths: EncryptedPaths::new(&["pages/private/".to_string()]),
        keys: Some(Arc::new(keys)),
    });
    let report = sync(&graph, &remote, &opts).unwrap();
    assert!(report.complete && report.errors.is_empty(), "{report:?}");

    let run = |password: &str| {
        Command::cargo_bin("notatnik")
            .unwrap()
            .env("XDG_CONFIG_HOME", tmp.path().join("cfg"))
            .env("XDG_DATA_HOME", tmp.path().join("data"))
            .env("NOTATNIK_PASSWORD", password)
            .env("NOTATNIK_WEBDAV_PASSWORD", &dav.password)
            .arg("verify")
            .arg(&graph)
            .args(["--url", &url, "--user", &dav.user])
            .assert()
    };
    let out = run("graph password").success();
    let text = String::from_utf8_lossy(&out.get_output().stdout).to_string();
    assert!(text.contains("the server copy matches"), "{text}");
    assert!(text.contains("stored encrypted"), "{text}");

    // a wrong password stops before anything is read
    run("wrong wrong wrong").failure().code(1);

    // an edit that was not synced yet: exit 2, the file is named
    std::fs::write(graph.join("pages/private/diary.md"), "- edited\n").unwrap();
    let out = run("graph password").failure().code(2);
    let text = String::from_utf8_lossy(&out.get_output().stdout).to_string();
    assert!(text.contains("pages/private/diary.md"), "{text}");
    assert_eq!(
        std::fs::read_to_string(graph.join("pages/private/diary.md")).unwrap(),
        "- edited\n"
    );
}
