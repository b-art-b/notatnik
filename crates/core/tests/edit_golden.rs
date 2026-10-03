//! Golden edit tests: `fixtures/edits/<op>/<case>/{before.md, op.json, after.md}`, byte-exact.
//! A case whose directory name ends in `-error` expects the op to be rejected (and has no after.md).
//! Run with `UPDATE_GOLDEN=1` to (re)write after.md files, then review the diff by hand.
mod common;

use notatnik_core::{BlockOp, apply, apply_patches, invert_patches, parse_page};
use std::collections::BTreeMap;

#[test]
fn golden_cases() {
    let root = common::fixtures_dir().join("edits");
    let update = std::env::var("UPDATE_GOLDEN").is_ok();
    let mut per_op: BTreeMap<String, usize> = BTreeMap::new();
    for op_dir in std::fs::read_dir(&root)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.is_dir())
    {
        for case in std::fs::read_dir(&op_dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.is_dir())
        {
            let name = format!(
                "{}/{}",
                op_dir.file_name().unwrap().to_string_lossy(),
                case.file_name().unwrap().to_string_lossy()
            );
            *per_op
                .entry(op_dir.file_name().unwrap().to_string_lossy().into_owned())
                .or_default() += 1;
            let before = String::from_utf8(std::fs::read(case.join("before.md")).unwrap()).unwrap();
            let op: BlockOp =
                serde_json::from_slice(&std::fs::read(case.join("op.json")).unwrap()).unwrap();
            let page = parse_page(&before);
            let result = apply(&before, &page, &op);
            if name.ends_with("-error") {
                assert!(result.is_err(), "{name}: expected an error");
                continue;
            }
            let result = result.unwrap_or_else(|e| panic!("{name}: {e}"));
            if update {
                std::fs::write(case.join("after.md"), &result.new_source).unwrap();
            }
            let after = String::from_utf8(std::fs::read(case.join("after.md")).unwrap_or_default())
                .unwrap();
            assert_eq!(
                result.new_source, after,
                "{name}: result differs from after.md"
            );
            assert_eq!(
                apply_patches(&before, &result.patches),
                result.new_source,
                "{name}: patches"
            );
            // inverse
            let inv = invert_patches(&result.patches);
            assert_eq!(
                apply_patches(&result.new_source, &inv),
                before,
                "{name}: inverse"
            );
            // reparse: no new diagnostics
            let reparsed = parse_page(&result.new_source);
            assert!(
                reparsed.diagnostics.len() <= page.diagnostics.len(),
                "{name}: new diagnostics {:?}",
                reparsed.diagnostics
            );
            assert!(
                reparsed.uncovered_non_blank(&result.new_source).is_empty(),
                "{name}: uncovered text"
            );
        }
    }
    for (op, n) in &per_op {
        assert!(*n >= 3, "operation {op} has only {n} golden cases");
    }
    assert_eq!(per_op.len(), 12, "every op needs golden cases: {per_op:?}");
}
