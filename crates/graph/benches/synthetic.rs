//! Criterion benchmarks on the synthetic 10,000-page graph (P1.39).
//! `cargo bench -p notatnik-graph` (scheduled in `scripts/check.sh --full` with `--quick`).

use std::path::PathBuf;
use std::time::Duration;

use chrono::NaiveDate;
use criterion::{Criterion, criterion_group, criterion_main};
use notatnik_core::{WeekStart, parse_page, parse_query};
use notatnik_graph::synth::{SynthOptions, generate};
use notatnik_index::{Index, IndexOptions, QueryContext};

fn graph_dir() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/synthetic/bench");
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

fn benches(c: &mut Criterion) {
    let dir = graph_dir();
    let mut idx = Index::open_in_memory(IndexOptions::default()).unwrap();
    idx.index_all(&dir).unwrap();

    let mut cold = c.benchmark_group("cold_index");
    cold.sample_size(10)
        .warm_up_time(Duration::from_millis(500))
        .measurement_time(Duration::from_secs(60));
    cold.bench_function("10k_pages", |b| {
        b.iter(|| {
            let mut i = Index::open_in_memory(IndexOptions::default()).unwrap();
            i.index_all(&dir).unwrap();
        });
    });
    cold.finish();

    let rel = "pages/bench-100.md";
    let body: String = (0..100)
        .map(|i| {
            format!("- block {i} about the project and [[work]] #urgent\n  - child {i} text\n")
        })
        .collect();
    let mut n = 0;
    c.bench_function("reindex_one_100_block_file", |b| {
        b.iter(|| {
            n += 1;
            std::fs::write(dir.join(rel), format!("{body}- edit {n}\n")).unwrap();
            idx.reindex_file(&dir, rel).unwrap();
        });
    });
    std::fs::remove_file(dir.join(rel)).unwrap();
    idx.reindex_file(&dir, rel).unwrap();

    c.bench_function("search_common_word", |b| {
        b.iter(|| idx.search("project", 20).unwrap())
    });
    c.bench_function("search_prefix", |b| {
        b.iter(|| idx.search("proj*", 20).unwrap())
    });

    let ctx = QueryContext {
        today: NaiveDate::from_ymd_opt(2026, 10, 3).unwrap(),
        week_start: WeekStart::Monday,
    };
    let mut g = c.benchmark_group("queries");
    for (name, q) in [
        (
            "tasks_due_soon",
            "task: todo, doing\ndeadline: < +7d\nsort: deadline asc",
        ),
        ("tag", "tag: urgent\ntask: any"),
        ("namespace_group", "ns: work\ntask: todo\ngroup: page"),
        (
            "property_sort",
            "prop.status: active\nsort: prop.rating desc\nlimit: 50",
        ),
        ("text", "text: budget\nlimit: 50"),
    ] {
        let parsed = parse_query(q).unwrap();
        g.bench_function(name, |b| b.iter(|| idx.run_query(&parsed, &ctx).unwrap()));
    }
    g.finish();

    idx.fuzzy_pages("wpa", 10).unwrap();
    c.bench_function("fuzzy_titles_3_chars", |b| {
        b.iter(|| idx.fuzzy_pages("wpa", 10).unwrap())
    });

    let big = std::fs::read_dir(dir.join("pages"))
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file())
        .max_by_key(|p| std::fs::metadata(p).map(|m| m.len()).unwrap_or(0))
        .map(|p| std::fs::read_to_string(p).unwrap())
        .unwrap();
    c.bench_function("parse_largest_page", |b| b.iter(|| parse_page(&big)));
}

criterion_group!(all, benches);
criterion_main!(all);
