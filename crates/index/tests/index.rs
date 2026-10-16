//! Index integration tests on fixture graphs (P1.19–P1.25).
mod common;

use common::*;
use notatnik_index::{Index, IndexOptions, SCHEMA_VERSION, SearchKind};

#[test]
fn new_database_has_tables() {
    let dir = tempfile::tempdir().unwrap();
    let (idx, info) =
        Index::open(&dir.path().join("index.sqlite"), IndexOptions::default()).unwrap();
    assert!(!info.rebuilt);
    let tables: Vec<String> = idx
        .connection()
        .prepare("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    for t in [
        "aliases", "blocks", "files", "pages", "props", "refs", "fts", "issues",
    ] {
        assert!(
            tables.iter().any(|x| x == t),
            "missing table {t}: {tables:?}"
        );
    }
    let mode: String = idx
        .connection()
        .query_row("PRAGMA journal_mode", [], |r| r.get(0))
        .unwrap();
    assert_eq!(mode, "wal");
    let fk: i64 = idx
        .connection()
        .query_row("PRAGMA foreign_keys", [], |r| r.get(0))
        .unwrap();
    assert_eq!(fk, 1);
}

#[test]
fn schema_version_mismatch_rebuilds() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("index.sqlite");
    {
        let (mut idx, _) = Index::open(&path, IndexOptions::default()).unwrap();
        idx.index_all(&fixture("basic")).unwrap();
        assert_eq!(idx.counts().unwrap().pages, 9);
        idx.connection()
            .pragma_update(None, "user_version", SCHEMA_VERSION - 1)
            .unwrap();
    }
    let (idx, info) = Index::open(&path, IndexOptions::default()).unwrap();
    assert!(info.rebuilt, "old version must trigger a rebuild");
    assert_eq!(idx.counts().unwrap().pages, 0, "rebuilt database is empty");
    let v: i64 = idx
        .connection()
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(v, SCHEMA_VERSION);
    drop(idx);
    let (_, info) = Index::open(&path, IndexOptions::default()).unwrap();
    assert!(!info.rebuilt);
}

#[test]
fn counts_match_expected_for_each_fixture_graph() {
    for g in ["basic", "namespaces", "refs", "tasks"] {
        let idx = indexed(g);
        let want: serde_json::Value = serde_json::from_slice(
            &std::fs::read(fixture(g).join("expected-counts.json")).unwrap(),
        )
        .unwrap();
        let got = serde_json::to_value(idx.counts().unwrap()).unwrap();
        assert_eq!(got, want, "{g}");
    }
}

#[test]
fn indexing_twice_gives_identical_rows() {
    for g in ["basic", "refs", "namespaces"] {
        let mut idx = indexed(g);
        let first = dump(&idx);
        idx.index_all(&fixture(g)).unwrap();
        assert_eq!(first, dump(&idx), "{g}");
        let other = indexed(g);
        assert_eq!(first, dump(&other), "{g} (fresh index)");
    }
}

#[test]
fn incremental_reindex_touches_only_one_page() {
    let dir = temp_graph("basic");
    let mut idx = Index::open_in_memory(IndexOptions::default()).unwrap();
    idx.index_all(dir.path()).unwrap();
    let pages: Vec<String> = idx
        .page_files()
        .unwrap()
        .into_iter()
        .map(|(n, _)| n)
        .collect();
    let before: Vec<(String, String)> = pages
        .iter()
        .map(|p| (p.clone(), page_checksum(&idx, p)))
        .collect();

    let file = dir.path().join("pages/people/anna.md");
    let text = std::fs::read_to_string(&file).unwrap().replace(
        "Prefers meetings in the morning",
        "Prefers meetings in the afternoon, see [[work/projects/alpha]]",
    );
    std::fs::write(&file, text).unwrap();
    let out = idx
        .reindex_file(dir.path(), "pages/people/anna.md")
        .unwrap();
    assert_eq!(out.page, "people/anna");

    for (p, sum) in before {
        let now = page_checksum(&idx, &p);
        if p == "people/anna" {
            assert_ne!(sum, now);
        } else {
            assert_eq!(sum, now, "page {p} must not change");
        }
    }
    assert!(
        idx.search("afternoon", 5)
            .unwrap()
            .iter()
            .any(|h| h.page == "people/anna")
    );
    assert!(idx.search("morning", 5).unwrap().is_empty());
    // a full rebuild gives the same content as the incremental result (row ids may differ)
    let mut fresh = Index::open_in_memory(IndexOptions::default()).unwrap();
    fresh.index_all(dir.path()).unwrap();
    assert_eq!(idx.counts().unwrap(), fresh.counts().unwrap());
    for p in &pages {
        assert_eq!(page_checksum(&idx, p), page_checksum(&fresh, p), "{p}");
    }
}

#[test]
fn deleting_a_linked_page_leaves_a_virtual_page_with_backlinks() {
    let dir = temp_graph("basic");
    let mut idx = Index::open_in_memory(IndexOptions::default()).unwrap();
    idx.index_all(dir.path()).unwrap();
    let before = idx.backlinks("people/anna").unwrap().len();
    assert!(before >= 3);
    std::fs::remove_file(dir.path().join("pages/people/anna.md")).unwrap();
    let out = idx
        .reindex_file(dir.path(), "pages/people/anna.md")
        .unwrap();
    assert!(out.removed);
    let info = idx.page_info("people/anna").unwrap().unwrap();
    assert!(info.is_virtual);
    assert_eq!(idx.backlinks("people/anna").unwrap().len(), before);
    // deleting a page nobody links to removes it entirely
    std::fs::remove_file(dir.path().join("pages/inbox.md")).unwrap();
    idx.reindex_file(dir.path(), "pages/inbox.md").unwrap();
    assert!(idx.page_info("inbox").unwrap().is_none());
}

#[test]
fn virtual_pages_for_missing_targets_and_gc() {
    let dir = temp_graph("namespaces");
    let mut idx = Index::open_in_memory(IndexOptions::default()).unwrap();
    idx.index_all(dir.path()).unwrap();
    assert!(
        idx.page_info("work/projects/gamma")
            .unwrap()
            .unwrap()
            .is_virtual
    );
    assert!(idx.page_info("x/y/z").unwrap().unwrap().is_virtual);
    // remove the last ref to the virtual pages
    std::fs::write(
        dir.path().join("pages/people/anna.md"),
        "- No links any more\n",
    )
    .unwrap();
    idx.reindex_file(dir.path(), "pages/people/anna.md")
        .unwrap();
    for gone in ["work/projects/gamma", "x/y/z", "x/y", "x"] {
        assert!(
            idx.page_info(gone).unwrap().is_none(),
            "{gone} should be garbage-collected"
        );
    }
    // namespace parents with real children stay
    assert!(idx.page_info("work/projects").unwrap().unwrap().is_virtual);
}

#[test]
fn deep_namespace_creates_virtual_ancestors() {
    let idx = indexed("namespaces");
    for a in ["a", "a/b", "a/b/c", "a/b/c/d"] {
        let info = idx
            .page_info(a)
            .unwrap()
            .unwrap_or_else(|| panic!("{a} missing"));
        assert!(info.is_virtual, "{a}");
    }
    assert_eq!(idx.namespace_children("a/b").unwrap(), ["a/b/c"]);
    assert_eq!(idx.namespace_children("work").unwrap(), ["work/projects"]);
    assert_eq!(
        idx.namespace_children("work/projects").unwrap(),
        [
            "work/projects/alpha",
            "work/projects/beta",
            "work/projects/gamma"
        ]
    );
}

#[test]
fn alias_resolution_and_case_insensitivity() {
    let idx = indexed("refs");
    let info = idx.page_info("SRC").unwrap().unwrap();
    assert_eq!(info.name, "source");
    assert!(!info.is_virtual);
    assert!(idx.page_info("the source").unwrap().is_some());
    assert!(idx.page_info("src").unwrap().is_some());
}

#[test]
fn backlinks_match_expected_json() {
    let idx = indexed("refs");
    let want: serde_json::Map<String, serde_json::Value> = serde_json::from_slice(
        &std::fs::read(fixture("refs").join("expected-backlinks.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(want.len(), 5);
    for (page, expected) in want {
        let got: Vec<serde_json::Value> = idx
            .backlinks(&page)
            .unwrap()
            .into_iter()
            .map(|b| serde_json::json!({"source_page": b.source_page, "block_path": b.block_path, "kind": b.kind, "breadcrumbs": b.breadcrumbs}))
            .collect();
        assert_eq!(
            serde_json::Value::Array(got),
            expected,
            "backlinks of {page}"
        );
    }
}

#[test]
fn backlinks_include_tags_and_namespaces_are_not_confused() {
    let idx = indexed("basic");
    let b = idx.backlinks("health").unwrap();
    assert_eq!(b.len(), 2);
    assert!(b.iter().all(|x| x.kind == "tag"));
    let b = idx.backlinks("project").unwrap();
    assert_eq!(b.len(), 1, "frontmatter tag is a page-level backlink");
    assert!(b[0].block_path.is_none());
}

#[test]
fn full_text_search_is_diacritic_and_case_insensitive() {
    let idx = indexed("basic");
    let has = |q: &str, needle: &str| {
        idx.search(q, 20)
            .unwrap()
            .iter()
            .any(|h| h.text.contains(needle))
    };
    assert!(has("zolw", "Żółw"));
    assert!(has("ZAŻÓŁĆ", "zażółć"));
    assert!(has("lodz", "Łódź"));
    assert!(has("łódź", "Łódź"));
    assert!(has("proj*", "projekcie"));
    assert!(
        !has("proj", "projekcie"),
        "without * it must be a whole word"
    );
    assert!(idx.search("", 5).unwrap().is_empty());
    assert!(idx.search("zzzzqqq", 5).unwrap().is_empty());
}

#[test]
fn search_highlights_and_content_without_syntax_noise() {
    let idx = indexed("basic");
    let hits = idx.search("collect", 10).unwrap();
    assert!(!hits.is_empty());
    let h = &hits[0];
    let (s, e) = h.highlights[0];
    assert_eq!(h.text[s..e].to_lowercase(), "collect");
    // the block id and link syntax are not searchable; link labels are
    assert!(idx.search("k3x9qa", 5).unwrap().is_empty());
    assert!(
        idx.search("kickoff", 5)
            .unwrap()
            .iter()
            .any(|h| h.page == "work/projects/alpha")
    );
    let polish = idx.search("żółw", 5).unwrap();
    assert_eq!(polish[0].highlights[0], (0, "Żółw".len()));
}

#[test]
fn title_matches_rank_above_body_matches() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("pages")).unwrap();
    std::fs::write(
        dir.path().join("pages/Alpha.md"),
        "- Nothing relevant here\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("pages/aaa.md"),
        "- We talked about alpha and alpha and alpha\n",
    )
    .unwrap();
    let mut idx = Index::open_in_memory(IndexOptions::default()).unwrap();
    idx.index_all(dir.path()).unwrap();
    let hits = idx.search("alpha", 10).unwrap();
    assert_eq!(hits[0].kind, SearchKind::Page);
    assert_eq!(hits[0].page, "Alpha");
    assert_eq!(hits[1].kind, SearchKind::Block);
}

#[test]
fn fuzzy_page_titles() {
    let dir = tempfile::tempdir().unwrap();
    for n in [
        "work/projects/alpha",
        "alphabet-soup",
        "zebra",
        "work/projects/beta",
        "reading/zażółć",
    ] {
        let p = dir.path().join(format!("pages/{n}.md"));
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, "- x\n").unwrap();
    }
    let mut idx = Index::open_in_memory(IndexOptions::default()).unwrap();
    idx.index_all(dir.path()).unwrap();
    let hits = idx.fuzzy_pages("wpa", 5).unwrap();
    assert!(
        hits.iter().any(|h| h.name == "work/projects/alpha"),
        "{hits:?}"
    );
    let hits = idx.fuzzy_pages("alpha", 5).unwrap();
    assert_eq!(hits[0].name, "work/projects/alpha", "{hits:?}");
    assert_eq!(hits[1].name, "alphabet-soup");
    assert!(
        idx.fuzzy_pages("zazolc", 5)
            .unwrap()
            .iter()
            .any(|h| h.name == "reading/zażółć")
    );
    // cache refreshes after changes
    std::fs::write(dir.path().join("pages/newpage.md"), "- x\n").unwrap();
    idx.reindex_file(dir.path(), "pages/newpage.md").unwrap();
    assert!(
        idx.fuzzy_pages("newpg", 5)
            .unwrap()
            .iter()
            .any(|h| h.name == "newpage")
    );
}

#[test]
fn reconcile_counts_changes_exactly() {
    let dir = temp_graph("basic");
    let mut idx = Index::open_in_memory(IndexOptions::default()).unwrap();
    idx.index_all(dir.path()).unwrap();
    let p = |s: &str| dir.path().join(s);
    // offline changes: modify 3, delete 1, add 1, touch 1 (mtime only)
    for f in [
        "pages/inbox.md",
        "pages/people/anna.md",
        "journals/2026/2026-10-01.md",
    ] {
        let t = std::fs::read_to_string(p(f)).unwrap();
        std::fs::write(p(f), format!("{t}- appended line\n")).unwrap();
    }
    std::fs::remove_file(p("journals/2026/2026-10-02.md")).unwrap();
    std::fs::write(p("pages/brand-new.md"), "- new page\n").unwrap();
    let touched = std::fs::OpenOptions::new()
        .write(true)
        .open(p("pages/work/projects.md"))
        .unwrap();
    touched
        .set_modified(std::time::SystemTime::now() + std::time::Duration::from_secs(5))
        .unwrap();
    drop(touched);

    let stats = idx.reconcile(dir.path()).unwrap();
    assert_eq!(stats.reindexed, 4, "3 modified + 1 new");
    assert_eq!(stats.added, 1);
    assert_eq!(stats.deleted, 1);
    assert_eq!(stats.touched, 1, "touched file is hashed but not reindexed");
    assert_eq!(
        stats.hashed, 4,
        "3 modified + 1 touched are hashed; the new file is just read"
    );
    // a second reconcile finds nothing
    let again = idx.reconcile(dir.path()).unwrap();
    assert_eq!(
        (again.reindexed, again.deleted, again.touched, again.hashed),
        (0, 0, 0, 0)
    );
    assert!(idx.search("appended", 10).unwrap().len() >= 3);
    assert!(idx.page_info("brand-new").unwrap().is_some());
}

#[test]
fn duplicate_block_ids_across_files() {
    let dir = temp_graph("refs");
    let mut idx = Index::open_in_memory(IndexOptions::default()).unwrap();
    idx.index_all(dir.path()).unwrap();
    // copy a block with its id into another file
    std::fs::write(
        dir.path().join("pages/zzz-copy.md"),
        "- Copy of the top block ^aaa111\n",
    )
    .unwrap();
    let out = idx.reindex_file(dir.path(), "pages/zzz-copy.md").unwrap();
    assert_eq!(out.duplicates.len(), 1);
    assert_eq!(out.duplicates[0].id, "aaa111");
    assert_eq!(out.duplicates[0].path, vec![0]);
    // the original keeps the id
    let (page, _, text) = idx.block_by_uid("aaa111").unwrap().unwrap();
    assert_eq!(page, "source");
    assert!(text.starts_with("Top block"));
    assert!(
        idx.issues()
            .unwrap()
            .iter()
            .any(|i| i.code == "duplicate-id")
    );

    // cold index: the file that sorts first by path keeps the id
    let mut fresh = Index::open_in_memory(IndexOptions::default()).unwrap();
    let dups = fresh.index_all(dir.path()).unwrap();
    assert_eq!(dups.len(), 1);
    assert_eq!(dups[0].0, "pages/zzz-copy.md");
    assert_eq!(fresh.block_by_uid("aaa111").unwrap().unwrap().0, "source");
    // reverse the order: name the copy so it sorts first
    std::fs::rename(
        dir.path().join("pages/zzz-copy.md"),
        dir.path().join("pages/aaa-copy.md"),
    )
    .unwrap();
    let dups = fresh.index_all(dir.path()).unwrap();
    assert_eq!(dups[0].0, "pages/source.md");
    assert_eq!(fresh.block_by_uid("aaa111").unwrap().unwrap().0, "aaa-copy");
}

#[test]
fn non_utf8_and_duplicate_page_files_are_reported_not_fatal() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("pages")).unwrap();
    std::fs::write(dir.path().join("pages/ok.md"), "- fine\n").unwrap();
    std::fs::write(
        dir.path().join("pages/bad.md"),
        [0x2d, 0x20, 0xff, 0xfe, b'\n'],
    )
    .unwrap();
    std::fs::write(dir.path().join("pages/Dup.md"), "- first\n").unwrap();
    std::fs::write(dir.path().join("pages/dup.md"), "- second\n").unwrap();
    let mut idx = Index::open_in_memory(IndexOptions::default()).unwrap();
    idx.index_all(dir.path()).unwrap();
    let codes: Vec<String> = idx.issues().unwrap().into_iter().map(|i| i.code).collect();
    assert!(codes.contains(&"not-utf8".to_string()), "{codes:?}");
    assert!(codes.contains(&"duplicate-page".to_string()), "{codes:?}");
    assert_eq!(idx.counts().unwrap().pages, 2);
}

#[test]
fn conflict_files_are_not_indexed() {
    let dir = temp_graph("basic");
    std::fs::write(
        dir.path()
            .join("pages/work/alpha (conflicted copy 2026-10-03 120000).md"),
        "- conflicted\n",
    )
    .unwrap();
    let files = notatnik_index::walk::scan(dir.path()).unwrap();
    assert_eq!(
        files.conflicts,
        ["pages/work/alpha (conflicted copy 2026-10-03 120000).md"]
    );
    let mut idx = Index::open_in_memory(IndexOptions::default()).unwrap();
    idx.index_all(dir.path()).unwrap();
    assert!(idx.search("conflicted", 5).unwrap().is_empty());
    assert_eq!(idx.counts().unwrap().pages, 9);
}

/// Removing a deep page leaves none of its virtual ancestors behind, whatever order the candidates come in
/// (a moved namespace used to leave `P/Capgemini/RfP` as a "no page" place in the topic view).
#[test]
fn removing_a_deep_page_collects_all_its_virtual_ancestors() {
    for round in 0..12 {
        let dir = tempfile::tempdir().unwrap();
        let rel = format!("pages/P{round}/Cap/RfP/cli/PDD.md");
        let abs = dir.path().join(&rel);
        std::fs::create_dir_all(abs.parent().unwrap()).unwrap();
        std::fs::write(&abs, "- note\n").unwrap();
        let mut idx = Index::open_in_memory(IndexOptions::default()).unwrap();
        idx.index_all(dir.path()).unwrap();
        for k in [
            format!("P{round}"),
            format!("P{round}/Cap"),
            format!("P{round}/Cap/RfP"),
            format!("P{round}/Cap/RfP/cli"),
        ] {
            assert!(idx.page_info(&k).unwrap().unwrap().is_virtual, "{k}");
        }
        std::fs::remove_file(&abs).unwrap();
        idx.reindex_file(dir.path(), &rel).unwrap();
        for k in [
            format!("P{round}/Cap/RfP/cli/PDD"),
            format!("P{round}/Cap/RfP/cli"),
            format!("P{round}/Cap/RfP"),
            format!("P{round}/Cap"),
            format!("P{round}"),
        ] {
            assert!(idx.page_info(&k).unwrap().is_none(), "{k} left behind");
        }
    }
}

/// SR-4: AND, OR, NOT / `-word`, parentheses, phrases and prefixes in the search box.
#[test]
fn search_understands_boolean_syntax() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("pages")).unwrap();
    std::fs::write(
        dir.path().join("pages/notes.md"),
        "- budget report draft\n- budget forecast final\n- annual plan report\n- budget and plan for the year\n- unrelated words\n",
    )
    .unwrap();
    let mut idx = Index::open_in_memory(IndexOptions::default()).unwrap();
    idx.index_all(dir.path()).unwrap();
    let texts = |q: &str| -> Vec<String> {
        let mut v: Vec<String> = idx
            .search(q, 20)
            .unwrap()
            .into_iter()
            .filter(|h| h.kind == SearchKind::Block)
            .map(|h| h.text)
            .collect();
        v.sort();
        v
    };
    assert_eq!(texts("budget report"), ["budget report draft"]);
    assert_eq!(texts("budget AND report"), ["budget report draft"]);
    assert_eq!(
        texts("forecast OR report"),
        [
            "annual plan report",
            "budget forecast final",
            "budget report draft"
        ]
    );
    assert_eq!(
        texts("forecast | draft"),
        ["budget forecast final", "budget report draft"]
    );
    assert_eq!(
        texts("budget -report"),
        ["budget and plan for the year", "budget forecast final"]
    );
    assert_eq!(
        texts("budget NOT (report OR forecast)"),
        ["budget and plan for the year"]
    );
    assert_eq!(
        texts("(budget OR annual) report"),
        ["annual plan report", "budget report draft"]
    );
    assert_eq!(texts("\"annual plan\""), ["annual plan report"]);
    assert_eq!(texts("\"annual report\""), Vec::<String>::new()); // a phrase is words next to each other
    assert_eq!(texts("repor* -draft"), ["annual plan report"]);
    // lowercase and/or are ordinary words
    assert_eq!(texts("budget and plan"), ["budget and plan for the year"]);
    // highlights cover the words that matched, not the excluded ones
    let hit = idx
        .search("budget -report", 5)
        .unwrap()
        .into_iter()
        .find(|h| h.text == "budget forecast final")
        .unwrap();
    assert_eq!(
        &hit.text[hit.highlights[0].0..hit.highlights[0].1],
        "budget"
    );
    // a mistake is explained
    let err = idx.search("(budget", 5).unwrap_err().to_string();
    assert!(err.contains("not closed"), "{err}");
    assert!(
        idx.search("-report", 5)
            .unwrap_err()
            .to_string()
            .contains("takes results away")
    );
}
