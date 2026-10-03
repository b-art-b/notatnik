//! Performance targets on the synthetic graph (NF-2, NF-3, NF-5). Slow: run with
//! `cargo test --release -p notatnik-graph --test perf -- --ignored --nocapture` (scripts/check.sh --full).

use std::path::PathBuf;
use std::time::{Duration, Instant};

use chrono::NaiveDate;
use notatnik_core::{WeekStart, parse_page, parse_query};
use notatnik_graph::synth::{SynthOptions, generate};
use notatnik_index::{Index, IndexOptions, QueryContext};

fn graph_dir() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("synthetic-10k");
    if !dir.join("pages").exists() {
        generate(
            &SynthOptions {
                seed: 1,
                pages: 10_000,
                journal_days: 730,
            },
            &dir,
        )
        .unwrap();
    }
    dir
}

fn avg<F: FnMut()>(n: u32, mut f: F) -> Duration {
    let t = Instant::now();
    for _ in 0..n {
        f();
    }
    t.elapsed() / n
}

#[test]
#[ignore = "slow: generates and indexes a 10,000-page graph"]
fn synthetic_graph_meets_performance_targets() {
    let dir = graph_dir();
    let mut idx = Index::open_in_memory(IndexOptions::default()).unwrap();

    let t = Instant::now();
    idx.index_all(&dir).unwrap();
    let cold = t.elapsed();
    let counts = idx.counts().unwrap();
    println!(
        "cold index: {cold:?} ({} pages, {} blocks)",
        counts.pages, counts.blocks
    );
    assert!(
        cold < Duration::from_secs(30),
        "NF-2: cold index took {cold:?}"
    );

    // NF-3: reindex one 100-block file
    let rel = "pages/perf-100.md";
    let body: String = (0..100).map(|i| format!("- block {i} about the project and [[work]] #urgent\n  - child {i} with zażółć text\n")).collect();
    std::fs::write(dir.join(rel), &body).unwrap();
    idx.reindex_file(&dir, rel).unwrap();
    let mut n = 0;
    let reindex = avg(20, || {
        n += 1;
        std::fs::write(dir.join(rel), format!("{body}- edit {n}\n")).unwrap();
        idx.reindex_file(&dir, rel).unwrap();
    });
    println!("reindex one 100-block file: {reindex:?}");
    assert!(
        reindex < Duration::from_millis(50),
        "NF-3: reindex took {reindex:?}"
    );
    std::fs::remove_file(dir.join(rel)).unwrap();
    idx.reindex_file(&dir, rel).unwrap();

    // NF-5: search
    for term in ["project", "the", "zażółć", "budget meeting", "proj*"] {
        let d = avg(10, || {
            idx.search(term, 20).unwrap();
        });
        println!("search {term:?}: {d:?}");
        assert!(
            d < Duration::from_millis(100),
            "NF-5: search {term:?} took {d:?}"
        );
    }

    // P1.29: five typical queries
    let ctx = QueryContext {
        today: NaiveDate::from_ymd_opt(2026, 10, 3).unwrap(),
        week_start: WeekStart::Monday,
    };
    for q in [
        "task: todo, doing\ndeadline: < +7d\nsort: deadline asc",
        "tag: urgent\ntask: any",
        "ns: work\ntask: todo\ngroup: page",
        "prop.status: active\nsort: prop.rating desc\nlimit: 50",
        "text: budget\nlimit: 50",
        "links: work\nlimit: 100",
    ] {
        let parsed = parse_query(q).unwrap();
        let d = avg(5, || {
            idx.run_query(&parsed, &ctx).unwrap();
        });
        println!("query {:?}: {d:?}", q.replace('\n', " | "));
        assert!(d < Duration::from_millis(100), "query {q:?} took {d:?}");
    }

    // P1.25: fuzzy titles
    idx.fuzzy_pages("wpa", 10).unwrap(); // load the cache
    let d = avg(20, || {
        idx.fuzzy_pages("wpa", 10).unwrap();
    });
    println!(
        "fuzzy 3-char query over {} titles: {d:?}",
        counts.pages + counts.virtual_pages
    );
    assert!(d < Duration::from_millis(10), "fuzzy took {d:?}");

    // P1.15: parse + rebuild from spans is byte-identical for every file of the synthetic graph
    fn all_md(dir: &std::path::Path, out: &mut Vec<PathBuf>) {
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                all_md(&p, out);
            } else if p.extension().is_some_and(|x| x == "md") {
                out.push(p);
            }
        }
    }
    let mut files = Vec::new();
    all_md(&dir, &mut files);
    for f in &files {
        let text = std::fs::read_to_string(f).unwrap();
        let page = parse_page(&text);
        assert_eq!(page.rebuild(&text), text, "{}", f.display());
        assert!(
            page.diagnostics.is_empty(),
            "{}: {:?}",
            f.display(),
            page.diagnostics
        );
    }
    println!("no-op rebuild verified for {} files", files.len());

    // parse a 2,000-block file
    let big = std::fs::read_to_string(
        std::fs::read_dir(dir.join("pages"))
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_file())
            .max_by_key(|p| std::fs::metadata(p).map(|m| m.len()).unwrap_or(0))
            .unwrap(),
    )
    .unwrap();
    let d = avg(10, || {
        parse_page(&big);
    });
    println!("parse a {} KB file: {d:?}", big.len() / 1024);
}
