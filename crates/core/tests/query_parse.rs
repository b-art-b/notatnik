//! Query parsing: valid queries (snapshots), error positions, canonical round trip (P1.16).
use notatnik_core::{parse_query, parse_query_partial};
use proptest::prelude::*;

const VALID: &[(&str, &str)] = &[
    ("open-tasks", "task: todo, doing"),
    (
        "kanban",
        "task: todo, doing\nns: work\ntag: urgent\n-tag: someday\ndeadline: < +7d\nsort: deadline asc\nview: kanban\ngroup: task",
    ),
    ("any-task", "task: any\n-task: done, canceled"),
    (
        "links-and-pages",
        "links: people/anna, [[a, b]]\npage: work/projects\nns: #work",
    ),
    (
        "props",
        "prop.status: active, paused\nprop.rating: >= 4\nprop.completed: contains 2026-10-03\nprop.owner: [[people/anna]]\n-prop.draft: any",
    ),
    (
        "dates",
        "scheduled: today..+7d\ndeadline: none, < today\n-scheduled: this-week",
    ),
    ("text-search", "text: żółw, rust language"),
    (
        "pages",
        "type: page\nprop.type: book\nsort: prop.rating desc, page\nlimit: 20\ncolumns: page, prop.rating, prop.author",
    ),
    (
        "table",
        "tag: book\nview: table\ngroup: prop.status\nlimit: 100",
    ),
    (
        "whitespace-and-crlf",
        "  task:   todo  \r\n\r\n   tag:x\r\n",
    ),
    ("empty", ""),
];

#[test]
fn valid_queries_snapshot() {
    for (name, text) in VALID {
        let q = parse_query(text).unwrap_or_else(|e| panic!("{name}: {e:?}"));
        insta::with_settings!({ snapshot_path => "snapshots/query", prepend_module_to_snapshot => false }, {
            insta::assert_json_snapshot!(*name, q);
        });
        // canonical text parses back to the same AST
        let again = parse_query(&q.to_text())
            .unwrap_or_else(|e| panic!("{name}: canonical text {:?}: {e:?}", q.to_text()));
        assert_eq!(q, again, "{name}");
    }
}

#[test]
fn error_positions() {
    // (query, line, column, message fragment)
    let cases: &[(&str, usize, usize, &str)] = &[
        ("tags: x", 1, 1, "unknown key `tags`"),
        ("task: todo\ndeadline: 2026-13-45", 2, 11, "invalid date"),
        ("deadline: ~ 3", 1, 11, "unknown operator"),
        ("tag:", 1, 5, "empty value"),
        ("sort: page\nsort: text", 2, 1, "duplicate `sort`"),
        ("-sort: page", 1, 1, "negation"),
        ("prop.x: >> 3", 1, 9, "unknown operator"),
        ("limit: 0", 1, 8, "positive"),
        ("hello", 1, 1, "expected `key: value`"),
        ("tag: a,,b", 1, 6, "empty value in the list"),
        ("task: toodo", 1, 7, "unknown task state"),
        (
            "task: todo\n\n  tag: ok\nview: Bad Name",
            4,
            7,
            "invalid view name",
        ),
        ("ns: ./x", 1, 5, "invalid page name"),
        ("sort: deadlin", 1, 7, "cannot sort by"),
        ("sort: page up", 1, 7, "asc"),
        ("group: nothing", 1, 8, "cannot group"),
        ("prop.: x", 1, 6, "invalid property key"),
        ("type: both", 1, 7, "unknown type"),
        ("scheduled: 2026-10-01..soon", 1, 24, "invalid date"),
    ];
    for (text, line, col, frag) in cases {
        let errs = parse_query(text).expect_err(text);
        let e = &errs[0];
        assert_eq!((e.line, e.column), (*line, *col), "{text:?}: {e}");
        assert!(e.message.contains(frag), "{text:?}: {e}");
    }
}

#[test]
fn suggestions() {
    let e = &parse_query("tags: x").unwrap_err()[0];
    assert_eq!(
        e.to_string(),
        "1:1: unknown key `tags`; did you mean `tag`?"
    );
    let e = &parse_query("sort: deadlin").unwrap_err()[0];
    assert!(e.suggestion.as_deref().unwrap().contains("deadline"));
    let e = &parse_query("deadline: sometime").unwrap_err()[0];
    assert!(e.suggestion.as_deref().unwrap().contains("YYYY-MM-DD"));
    let e = &parse_query("property.status: x").unwrap_err()[0];
    assert!(e.message.contains("unknown key"));
}

#[test]
fn partial_parse_keeps_good_clauses() {
    let (q, errs) = parse_query_partial("tag: a\ntags: b\ntask: todo");
    assert_eq!(q.filters.len(), 2);
    assert_eq!(errs.len(), 1);
    assert_eq!(errs[0].line, 2);
}

fn clause() -> impl Strategy<Value = String> {
    let name = "[a-z][a-z0-9]{0,6}(/[a-z][a-z0-9]{0,6}){0,2}";
    let date = prop_oneof![
        Just("today".to_string()),
        Just("tomorrow".to_string()),
        Just("this-week".to_string()),
        Just("2026-10-03".to_string()),
        (
            1u32..30,
            prop_oneof![Just('d'), Just('w'), Just('m'), Just('y')],
            prop_oneof![Just('+'), Just('-')]
        )
            .prop_map(|(n, u, s)| format!("{s}{n}{u}")),
    ];
    let dcond = prop_oneof![
        Just("none".to_string()),
        Just("any".to_string()),
        date.clone().prop_map(|d| d),
        date.clone().prop_map(|d| format!("< {d}")),
        date.clone().prop_map(|d| format!(">= {d}")),
        (date.clone(), date.clone()).prop_map(|(a, b)| format!("{a}..{b}")),
    ];
    let pcond = prop_oneof![
        Just("any".to_string()),
        "[a-z]{3,8}".prop_filter("keywords", |s| !matches!(
            s.as_str(),
            "any" | "none" | "contains" | "today" | "yesterday" | "tomorrow"
        )),
        (0u32..500).prop_map(|n| format!(">= {n}")),
        (0u32..500).prop_map(|n| format!("!= {n}.5")),
        date.clone().prop_map(|d| format!("< {d}")),
        "[a-z]{3,8}"
            .prop_filter("keywords", |s| !matches!(s.as_str(), "any" | "none"))
            .prop_map(|s| format!("contains {s}")),
        name.prop_map(|n| format!("[[{n}]]")),
    ];
    prop_oneof![
        (
            any::<bool>(),
            prop::collection::vec(
                prop_oneof![
                    Just("todo"),
                    Just("doing"),
                    Just("done"),
                    Just("any"),
                    Just("none")
                ],
                1..3
            )
        )
            .prop_map(|(neg, v)| format!(
                "{}task: {}",
                if neg { "-" } else { "" },
                v.join(", ")
            )),
        (any::<bool>(), prop::collection::vec(name, 1..3)).prop_map(|(neg, v)| format!(
            "{}tag: {}",
            if neg { "-" } else { "" },
            v.join(", ")
        )),
        prop::collection::vec(name, 1..3).prop_map(|v| format!("ns: {}", v.join(", "))),
        prop::collection::vec(name, 1..3).prop_map(|v| format!("links: {}", v.join(", "))),
        name.prop_map(|n| format!("page: {n}")),
        prop::collection::vec(dcond.clone(), 1..3)
            .prop_map(|v| format!("deadline: {}", v.join(", "))),
        (any::<bool>(), prop::collection::vec(dcond, 1..3)).prop_map(|(neg, v)| format!(
            "{}scheduled: {}",
            if neg { "-" } else { "" },
            v.join(", ")
        )),
        ("[a-z]{1,6}", prop::collection::vec(pcond, 1..3))
            .prop_map(|(k, v)| format!("prop.{k}: {}", v.join(", "))),
        prop::collection::vec("[a-zA-Z]{2,8}", 1..3)
            .prop_map(|v| format!("text: {}", v.join(", "))),
    ]
}

proptest! {
    #[test]
    fn ast_text_ast_round_trip(
        filters in prop::collection::vec(clause(), 0..6),
        page_type in any::<bool>(),
        sort in proptest::option::of(prop::collection::vec((prop_oneof![Just("deadline"), Just("page"), Just("prop.rating"), Just("priority")], any::<bool>()), 1..3)),
        group in proptest::option::of(prop_oneof![Just("page"), Just("task"), Just("prop.status"), Just("deadline")]),
        limit in proptest::option::of(1usize..1000),
        view in proptest::option::of(prop_oneof![Just("list"), Just("table"), Just("kanban"), Just("my-plugin.view")]),
        columns in prop::collection::vec("[a-z]{2,6}", 0..3),
    ) {
        let mut text = filters.join("\n");
        text.push('\n');
        if page_type { text.push_str("type: page\n"); }
        if let Some(s) = sort {
            let keys: Vec<String> = s.iter().map(|(k, d)| format!("{k} {}", if *d { "desc" } else { "asc" })).collect();
            text.push_str(&format!("sort: {}\n", keys.join(", ")));
        }
        if let Some(g) = group { text.push_str(&format!("group: {g}\n")); }
        if let Some(l) = limit { text.push_str(&format!("limit: {l}\n")); }
        if let Some(v) = view { text.push_str(&format!("view: {v}\n")); }
        if !columns.is_empty() { text.push_str(&format!("columns: {}\n", columns.join(", "))); }
        let q = parse_query(&text).map_err(|e| TestCaseError::fail(format!("{text}\n{e:?}")))?;
        let canonical = q.to_text();
        let q2 = parse_query(&canonical).map_err(|e| TestCaseError::fail(format!("{canonical}\n{e:?}")))?;
        prop_assert_eq!(q, q2);
    }
}

#[test]
fn direct_clause_parses_and_round_trips() {
    let q = notatnik_core::parse_query("links: a\ndirect: yes").unwrap();
    assert!(q.direct);
    assert_eq!(notatnik_core::parse_query(&q.to_text()).unwrap(), q);
    assert!(!notatnik_core::parse_query("links: a").unwrap().direct);
    assert!(
        !notatnik_core::parse_query("links: a\ndirect: no")
            .unwrap()
            .direct
    );
    assert!(notatnik_core::parse_query("direct: maybe").is_err());
    assert!(notatnik_core::parse_query("direct: yes\ndirect: no").is_err());
}

#[test]
fn children_clause_parses_and_round_trips() {
    let q = notatnik_core::parse_query("task: todo\nchildren: yes").unwrap();
    assert!(q.children);
    assert_eq!(notatnik_core::parse_query(&q.to_text()).unwrap(), q);
    assert!(!notatnik_core::parse_query("task: todo").unwrap().children);
    assert!(!notatnik_core::parse_query("children: no").unwrap().children);
    assert!(notatnik_core::parse_query("children: maybe").is_err());
    assert!(notatnik_core::parse_query("children: yes\nchildren: no").is_err());
}

#[test]
fn properties_clause_parses_and_round_trips() {
    let p = |t: &str| notatnik_core::parse_query(t).unwrap().properties;
    assert_eq!(p("task: todo"), None);
    assert_eq!(p("properties: all"), None);
    assert_eq!(p("properties: none"), Some(vec![]));
    assert_eq!(
        p("properties: status, owner, status"),
        Some(vec!["status".into(), "owner".into()])
    );
    for t in ["properties: none", "properties: status, owner"] {
        let q = notatnik_core::parse_query(t).unwrap();
        assert_eq!(notatnik_core::parse_query(&q.to_text()).unwrap(), q);
    }
    assert!(notatnik_core::parse_query("properties: Bad Key").is_err());
    assert!(notatnik_core::parse_query("properties: none\nproperties: a").is_err());
    assert!(notatnik_core::parse_query("-properties: none").is_err());
}

#[test]
fn inherit_clause_parses_and_round_trips() {
    let q = notatnik_core::parse_query("prop.client: Acme\ninherit: client, type").unwrap();
    assert_eq!(q.inherit, ["client", "type"]);
    let again = notatnik_core::parse_query(&q.to_text()).unwrap();
    assert_eq!(again, q);
    assert!(notatnik_core::parse_query("inherit: Bad Key").is_err());
    assert!(notatnik_core::parse_query("inherit: a\ninherit: b").is_err());
}

/// Every example offered by `/query` and listed in Settings (`ui/src/lib/queryExamples.json`) parses, with the
/// placeholders the way the app fills them in.
#[test]
fn example_queries_parse() {
    let json = include_str!("../../../ui/src/lib/queryExamples.json");
    let list: serde_json::Value = serde_json::from_str(json).unwrap();
    let list = list.as_array().unwrap();
    assert!(list.len() >= 10);
    for e in list {
        let id = e["id"].as_str().unwrap();
        assert!(!e["title"].as_str().unwrap().is_empty(), "{id}");
        let q = e["query"]
            .as_str()
            .unwrap()
            .replace("{{page}}", "[[series/x]]")
            .replace("{{namespace}}", "x")
            .replace("{{type}}", "series");
        assert!(
            !q.contains("{{"),
            "{id}: a placeholder the test does not fill in"
        );
        notatnik_core::parse_query(&q).unwrap_or_else(|err| panic!("{id}\n{q}\n{err:?}"));
    }
}
