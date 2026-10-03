//! Query execution against `fixtures/graphs/queries` (P1.27–P1.29).
mod common;

use chrono::NaiveDate;
use common::*;
use notatnik_core::{WeekStart, parse_query};
use notatnik_index::{Index, IndexOptions, QueryContext};

fn ctx() -> QueryContext {
    QueryContext {
        today: NaiveDate::from_ymd_opt(2026, 10, 3).unwrap(),
        week_start: WeekStart::Monday,
    }
}

fn run(idx: &Index, text: &str) -> Vec<String> {
    let q = parse_query(text).unwrap_or_else(|e| panic!("{text}: {e:?}"));
    idx.run_query(&q, &ctx())
        .unwrap()
        .rows
        .into_iter()
        .map(|r| r.id)
        .collect()
}

#[test]
fn every_query_file_returns_its_expected_ids_in_order() {
    let idx = indexed("queries");
    let dir = fixture("queries");
    let mut n = 0;
    let mut files: Vec<_> = std::fs::read_dir(dir.join("queries"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    files.sort();
    for f in files {
        let name = f.file_stem().unwrap().to_string_lossy().into_owned();
        let text = std::fs::read_to_string(&f).unwrap();
        let want: Vec<String> = serde_json::from_slice(
            &std::fs::read(dir.join("expected").join(format!("{name}.json"))).unwrap(),
        )
        .unwrap();
        assert_eq!(run(&idx, &text), want, "query {name}:\n{text}");
        n += 1;
    }
    assert!(n >= 25, "only {n} queries");
}

#[test]
fn ns_prefix_trap_and_page_clause() {
    let idx = indexed("queries");
    let ids = run(&idx, "ns: work");
    assert!(ids.iter().all(|i| !i.starts_with("workshop")));
    assert!(ids.iter().any(|i| i.starts_with("work/projects/alpha")));
    let ids = run(&idx, "ns: workshop");
    assert_eq!(ids, ["workshop/tasks#0"]);
    let ids = run(&idx, "page: work");
    assert!(ids.is_empty(), "no page named exactly `work` has blocks");
    let ids = run(&idx, "ns: work/projects\ntask: any");
    assert!(ids.iter().all(|i| i.starts_with("work/projects/")));
}

#[test]
fn sql_injection_is_a_literal() {
    let idx = indexed("queries");
    let before = idx.counts().unwrap();
    for text in [
        "tag: x'); DROP TABLE blocks; --",
        "text: x'); DROP TABLE blocks; --",
        "prop.status: '; DROP TABLE blocks; --",
        "ns: a' OR '1'='1",
        "links: \" OR 1=1 --",
        "prop.x: contains %_\\",
    ] {
        let q = parse_query(text).unwrap();
        let r = idx.run_query(&q, &ctx());
        assert!(r.is_ok(), "{text}: {:?}", r.err());
        assert!(r.unwrap().rows.is_empty(), "{text} must match nothing");
    }
    assert_eq!(idx.counts().unwrap(), before, "tables intact");
    // the SQL text never contains user values
    let q = parse_query("tag: zzmarker\nprop.status: yymarker").unwrap();
    let c = idx.compile_query(&q, &ctx()).unwrap();
    assert!(!c.sql.contains("zzmarker") && !c.sql.contains("yymarker"));
}

#[test]
fn like_wildcards_in_contains_are_escaped() {
    let idx = indexed("queries");
    assert!(run(&idx, "prop.author: contains %").is_empty());
    assert_eq!(run(&idx, "prop.author: contains herb"), ["reading/books#0"]);
}

#[test]
fn group_labels_and_order() {
    let idx = indexed("queries");
    let q = parse_query("task: any\ngroup: task").unwrap();
    let rows = idx.run_query(&q, &ctx()).unwrap().rows;
    let labels: Vec<Option<String>> = rows.iter().map(|r| r.group.clone()).collect();
    let mut order: Vec<String> = Vec::new();
    for l in labels.into_iter().flatten() {
        if order.last() != Some(&l) {
            order.push(l);
        }
    }
    assert_eq!(order, ["doing", "todo", "waiting", "done", "canceled"]);

    let q = parse_query("task: todo\ngroup: page").unwrap();
    let rows = idx.run_query(&q, &ctx()).unwrap().rows;
    assert_eq!(rows[0].group.as_deref(), Some("2026-10-02"));
    assert_eq!(
        rows.last().unwrap().group.as_deref(),
        Some("workshop/tasks")
    );

    let q = parse_query("prop.rating: any\ngroup: prop.status").unwrap();
    let rows = idx.run_query(&q, &ctx()).unwrap().rows;
    // groups ordered by value, rows without the property last
    let g: Vec<Option<String>> = rows.iter().map(|r| r.group.clone()).collect();
    assert_eq!(g.first().unwrap().as_deref(), Some("done"));
    assert_eq!(g.last().unwrap(), &None);

    let q = parse_query("task: any\ngroup: deadline").unwrap();
    let rows = idx.run_query(&q, &ctx()).unwrap().rows;
    assert_eq!(rows.first().unwrap().group.as_deref(), Some("2026-09-01"));
    assert_eq!(rows.last().unwrap().group, None);
}

#[test]
fn rows_carry_task_fields_and_props() {
    let idx = indexed("queries");
    let q = parse_query("page: reading/books").unwrap();
    let rows = idx.run_query(&q, &ctx()).unwrap().rows;
    assert_eq!(
        rows[0].props.get("author").map(String::as_str),
        Some("Frank Herbert")
    );
    assert_eq!(
        rows[2].props.get("completed").map(String::as_str),
        Some("2026-09-20")
    );
    let q = parse_query("tag: urgent\npage: 2026-10-03").unwrap();
    let r = &idx.run_query(&q, &ctx()).unwrap().rows[0];
    assert_eq!(r.deadline.as_deref(), Some("2026-10-03"));
    assert_eq!(r.deadline_time.as_deref(), Some("17:00"));
    assert_eq!(r.task_state.as_deref(), Some("todo"));
}

#[test]
fn today_and_week_start_are_injected() {
    let idx = indexed("queries");
    let q = parse_query("scheduled: this-week").unwrap();
    let mut c = ctx();
    assert_eq!(idx.run_query(&q, &c).unwrap().rows.len(), 2);
    c.today = NaiveDate::from_ymd_opt(2026, 10, 5).unwrap();
    assert_eq!(
        idx.run_query(&q, &c)
            .unwrap()
            .rows
            .iter()
            .map(|r| r.id.as_str())
            .collect::<Vec<_>>(),
        ["work/projects/alpha#1"]
    );
    let q = parse_query("deadline: < today\ntask: any").unwrap();
    c.today = NaiveDate::from_ymd_opt(2026, 9, 2).unwrap();
    assert_eq!(
        idx.run_query(&q, &c).unwrap().rows.len(),
        1,
        "only the 2026-09-01 kickoff"
    );
}

#[test]
fn page_queries_reject_block_only_clauses() {
    let idx = indexed("queries");
    for t in [
        "type: page\ntask: todo",
        "type: page\ndeadline: today",
        "type: page\nsort: deadline asc",
        "type: page\ngroup: task",
    ] {
        let q = parse_query(t).unwrap();
        assert!(idx.run_query(&q, &ctx()).is_err(), "{t}");
    }
}

#[test]
fn alias_in_filters() {
    let idx = indexed("refs");
    // `links: src` must find blocks that link to `source` under any of its names
    let q = parse_query("links: src").unwrap();
    let ids: Vec<String> = idx
        .run_query(&q, &ctx())
        .unwrap()
        .rows
        .into_iter()
        .map(|r| r.id)
        .collect();
    assert_eq!(
        ids,
        ["linker#2", "linker#3", "linker#4", "other#0.0", "other#1"]
    );
}

#[test]
fn sql_snapshots_for_representative_queries() {
    let idx = indexed("queries");
    let texts = [
        "task: todo, doing\nns: work\ntag: urgent\n-tag: someday\ndeadline: < +7d\nsort: deadline asc\ngroup: task",
        "prop.rating: >= 4, any\n-prop.status: draft\nsort: prop.rating desc",
        "links: people/anna\ntext: parser, spec\nlimit: 5",
        "scheduled: today..+7d, none\nsort: priority asc, page desc",
        "type: page\ntag: project\nprop.type: person\nsort: page",
        "page: work/tasks, work\n-ns: workshop\ngroup: page",
        "prop.completed: contains 2026-10-03\ngroup: prop.status",
        "-task: any\n-deadline: any",
        "task: none, done\nsort: text desc",
        "deadline: this-week, 2026-12-01\n-scheduled: next-week",
    ];
    for (i, t) in texts.iter().enumerate() {
        let q = parse_query(t).unwrap();
        let c = idx.compile_query(&q, &ctx()).unwrap();
        let dump = format!("{t}\n---\n{}\n---\n{:?}", c.sql, c.params);
        insta::with_settings!({ snapshot_path => "snapshots/sql", prepend_module_to_snapshot => false }, {
            insta::assert_snapshot!(format!("q{:02}", i + 1), dump);
        });
    }
}

#[test]
fn query_plans_use_indexes() {
    let idx = indexed("queries");
    for t in [
        "task: todo",
        "ns: work",
        "tag: urgent",
        "deadline: < +7d",
        "prop.status: draft",
    ] {
        let q = parse_query(t).unwrap();
        let c = idx.compile_query(&q, &ctx()).unwrap();
        let mut st = idx
            .connection()
            .prepare(&format!("EXPLAIN QUERY PLAN {}", c.sql))
            .unwrap();
        let plan: Vec<String> = st
            .query_map(rusqlite::params_from_iter(c.params.iter()), |r| {
                r.get::<_, String>(3)
            })
            .unwrap()
            .map(Result::unwrap)
            .collect();
        assert!(
            plan.iter()
                .any(|l| l.contains("INDEX") || l.contains("PRIMARY KEY")),
            "{t}: {plan:?}"
        );
        println!("{t}\n  {}", plan.join("\n  "));
    }
}

#[test]
fn compile_does_not_need_data() {
    let idx = Index::open_in_memory(IndexOptions::default()).unwrap();
    let q = parse_query("task: todo").unwrap();
    assert!(idx.run_query(&q, &ctx()).unwrap().rows.is_empty());
}
