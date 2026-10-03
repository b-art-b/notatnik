//! Targeted tests for frontmatter, properties, tasks and block IDs (P1.5, P1.7, P1.9, P1.10).
use std::collections::HashSet;

use notatnik_core::{
    ParseOptions, PropType, PropertyValue, TaskState, ids, parse_page, parse_page_with,
};

#[test]
fn frontmatter_valid() {
    let p = parse_page(
        "---\ntitle: T\naliases: [a, b]\ntags:\n  - x\n  - y\nstatus: active\nrating: 4\nstarted: 2026-09-01\nowner: \"[[people/anna]]\"\n---\n- b\n",
    );
    let fm = p.frontmatter.unwrap();
    assert!(fm.valid && p.diagnostics.is_empty());
    assert_eq!(fm.title.as_deref(), Some("T"));
    assert_eq!(fm.aliases, ["a", "b"]);
    assert_eq!(fm.tags, ["x", "y"]);
    let get = |k: &str| {
        fm.properties
            .iter()
            .find(|p| p.key == k)
            .unwrap()
            .value
            .clone()
    };
    assert_eq!(get("status"), PropertyValue::Text("active".into()));
    assert_eq!(get("rating"), PropertyValue::Number(4.0));
    assert!(matches!(get("started"), PropertyValue::Date(_)));
    assert_eq!(
        get("owner"),
        PropertyValue::Links(vec!["people/anna".into()])
    );
    assert_eq!(fm.entries.len(), 7);
}

#[test]
fn frontmatter_empty_invalid_and_hr() {
    let p = parse_page("---\n---\n- a\n");
    assert!(p.frontmatter.as_ref().unwrap().valid && p.diagnostics.is_empty());
    assert!(p.frontmatter.unwrap().properties.is_empty());

    let src = "---\ntitle: [x\n---\n- a\n";
    let p = parse_page(src);
    assert!(!p.frontmatter.as_ref().unwrap().valid);
    assert_eq!(p.diagnostics.len(), 1);
    assert!(
        p.frontmatter
            .as_ref()
            .unwrap()
            .span
            .contains_span(&p.diagnostics[0].span)
    );
    assert_eq!(p.rebuild(src), src);

    let p = parse_page("- a\n- b\n- c\n- d\n---\n- e\n");
    assert!(p.frontmatter.is_none());
}

#[test]
fn tasks() {
    let src =
        "- TODO x\n- todo x\n- TODOx x\n- [ ] x\n- DONE\n- WAITING w\n- CANCELED c\n- DOING d\n";
    let p = parse_page(src);
    let states: Vec<Option<TaskState>> = p
        .blocks
        .iter()
        .map(|b| b.task.as_ref().map(|t| t.state))
        .collect();
    use TaskState::*;
    assert_eq!(
        states,
        [
            Some(Todo),
            None,
            None,
            None,
            Some(Done),
            Some(Waiting),
            Some(Canceled),
            Some(Doing)
        ]
    );
    assert_eq!(
        p.blocks[0].task.as_ref().unwrap().marker_span.slice(src),
        "TODO"
    );
}

#[test]
fn task_properties_and_diagnostics() {
    let src = "- TODO t\n  scheduled:: 2026-10-05 09:30\n  deadline:: 2026-13-45\n  priority:: urgent\n  repeat:: sometimes\n- TODO ok\n  repeat:: every 2w\n  priority:: low\n  done:: 2026-10-02T18:20\n";
    let p = parse_page(src);
    let t = p.blocks[0].task.as_ref().unwrap();
    assert_eq!(t.scheduled.unwrap().to_string(), "2026-10-05 09:30");
    assert!(t.deadline.is_none() && t.priority.is_none() && t.repeat.is_none());
    let codes: Vec<_> = p.diagnostics.iter().map(|d| d.code).collect();
    assert_eq!(
        codes,
        ["invalid-date", "invalid-priority", "invalid-repeat"]
    );
    let t2 = p.blocks[1].task.as_ref().unwrap();
    assert!(t2.repeat.is_some() && t2.done.is_some());
}

#[test]
fn property_edges() {
    let src = "- First line\n  a:: 1\n  b:: 2\n  Normal content\n  c:: 3\n- Fence\n  ```\n  d:: 4\n  ```\n- Blank\n  a:: 1\n\n  b:: 2\n";
    let p = parse_page(src);
    let keys = |i: usize| {
        p.blocks[i]
            .properties
            .iter()
            .map(|x| x.key.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(keys(0), ["a", "b"], "stops at the first non-property line");
    assert!(keys(1).is_empty(), "inside a fence");
    assert_eq!(keys(2), ["a"], "a blank line ends the property zone");
}

#[test]
fn property_types_from_config() {
    let mut opts = ParseOptions::default();
    opts.property_types
        .insert("rating".into(), PropType::Number);
    opts.property_types.insert("zip".into(), PropType::Text);
    let p = parse_page_with("- b\n  rating:: x\n  zip:: 2026-10-05\n", &opts);
    assert_eq!(p.diagnostics[0].code, "property-type");
    assert_eq!(
        p.blocks[0].properties[1].value,
        PropertyValue::Text("2026-10-05".into())
    );
}

#[test]
fn block_id_generation() {
    let mut existing = HashSet::new();
    let mut n = 0u64;
    let mut rng = || {
        n += 1;
        n * 7919
    };
    let first = ids::generate_block_id(&HashSet::new(), &mut || 7919);
    existing.insert(first.clone());
    let second = ids::generate_block_id(&existing, &mut rng_from([7919, 15838]));
    assert_ne!(first, second);
    assert!(ids::is_valid_block_id(&second));
    let _ = &mut rng;
}

fn rng_from<const N: usize>(vals: [u64; N]) -> impl FnMut() -> u64 {
    let mut i = 0;
    move || {
        let v = vals[i.min(N - 1)];
        i += 1;
        v
    }
}

#[test]
fn id_excluded_from_search_text() {
    let src = "- See [[work/alpha|the alpha]] and #tag ((abc123)) ^k3x9qa\n  status:: active\n  more [[b]] text\n";
    let p = parse_page(src);
    assert_eq!(
        p.blocks[0].search_text(src),
        "See the alpha and tag\nmore b text"
    );
    assert_eq!(
        p.blocks[0].edit_text(src),
        "See [[work/alpha|the alpha]] and #tag ((abc123))\nstatus:: active\nmore [[b]] text"
    );
}
