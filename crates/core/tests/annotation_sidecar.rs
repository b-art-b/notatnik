//! PA-2, PA-3, PA-9: the sidecar page of a PDF is an ordinary page; its shape parses as expected and a no-op edit
//! leaves it byte-identical (ST-3). `fixtures/annotations/hls-report.md`.
mod common;

use notatnik_core::{BlockOp, BlockRef, apply, parse_page};

fn fixture() -> String {
    std::fs::read_to_string(common::fixtures_dir().join("annotations/hls-report.md")).unwrap()
}

#[test]
fn the_sidecar_parses() {
    let text = fixture();
    let page = parse_page(&text);
    let fm = page.frontmatter.as_ref().unwrap();
    assert!(fm.valid);
    let get = |k: &str| {
        fm.properties
            .iter()
            .find(|p| p.key == k)
            .unwrap_or_else(|| panic!("no {k}"))
            .raw
            .clone()
    };
    assert_eq!(get("file"), "assets/2026/report.pdf");
    assert_eq!(get("kind"), "annotations");
    assert_eq!(get("sha"), "9f2c41d07ab3e815");
    assert_eq!(page.blocks.len(), 2);
    let first = &page.blocks[0];
    assert_eq!(first.block_id.as_ref().unwrap().id, "a1b2c3");
    let prop = |b: &notatnik_core::Block, k: &str| {
        b.properties
            .iter()
            .find(|p| p.key == k)
            .unwrap_or_else(|| panic!("no {k}"))
            .raw
            .clone()
    };
    assert_eq!(prop(first, "hl-page"), "12");
    assert_eq!(prop(first, "hl-from"), "1042");
    assert_eq!(prop(first, "hl-to"), "1187");
    assert_eq!(prop(first, "hl-context"), "…text before… | …text after…");
    assert_eq!(first.children.len(), 1, "the comment is a child block");
    assert_eq!(
        prop(&page.blocks[1], "hl-rect"),
        "0.2000,0.4000,0.6000,0.2500"
    );
    assert!(page.diagnostics.is_empty(), "{:?}", page.diagnostics);
    assert!(page.uncovered_non_blank(&text).is_empty());
}

#[test]
fn a_no_op_edit_changes_nothing() {
    let text = fixture();
    let page = parse_page(&text);
    for (i, b) in page.blocks.iter().enumerate() {
        let op = BlockOp::UpdateText {
            block: BlockRef::at(&[i]),
            text: b.edit_text(&text),
        };
        assert_eq!(apply(&text, &page, &op).unwrap().new_source, text);
    }
    let same = BlockOp::SetProperty {
        block: BlockRef::at(&[0]),
        key: "hl-color".into(),
        value: "yellow".into(),
    };
    assert_eq!(apply(&text, &page, &same).unwrap().new_source, text);
}
