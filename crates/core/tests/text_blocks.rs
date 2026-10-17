//! Unindented text between bullets that follows a blank line is a block of its own (BlockKind::Text), as other
//! Markdown editors (Nextcloud Text, Obsidian) write pages.
use notatnik_core::{BlockKind, BlockOp, BlockRef, apply, parse_page};

const PAGE: &str = "# Title\n\n* first\n* second\n\n## Heading ✅\n\n* third\n\n```\ncode\n\nmore\n```\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n[^1]:\n    - nested item\n      continues\n\n[^2]: later\n";

fn kinds(src: &str) -> Vec<(BlockKind, String)> {
    let p = parse_page(src);
    p.blocks
        .iter()
        .map(|b| (b.kind, b.first_line.slice(src).to_string()))
        .collect()
}

#[test]
fn text_after_a_blank_line_is_its_own_block_and_bullets_start_again_at_the_top() {
    let k = kinds(PAGE);
    let shape: Vec<(BlockKind, &str)> = k.iter().map(|(a, b)| (*a, b.as_str())).collect();
    assert_eq!(
        shape,
        [
            (BlockKind::Bullet, "first"),
            (BlockKind::Bullet, "second"),
            (BlockKind::Text, "## Heading ✅"),
            (BlockKind::Bullet, "third"),
            (BlockKind::Text, "```"),
        ]
    );
    // the last text block holds the code, the table and both footnotes (an indented list item belongs to the text)
    let p = parse_page(PAGE);
    let last = p.blocks.last().unwrap();
    assert!(last.children.is_empty());
    let text = last.edit_text(PAGE);
    assert!(
        text.starts_with("```\ncode\n\nmore\n```\n\n| a | b |"),
        "{text}"
    );
    assert!(
        text.contains("\n    - nested item\n      continues\n"),
        "indentation is kept: {text}"
    );
    assert!(text.ends_with("[^2]: later"));
    // nothing is lost: the file is rebuilt byte for byte, and every non-blank line is covered
    assert_eq!(p.rebuild(PAGE), PAGE);
    assert!(p.uncovered_non_blank(PAGE).is_empty());
    assert!(p.preamble.is_some());
}

#[test]
fn text_right_after_a_bullet_without_a_blank_line_is_still_a_continuation() {
    let src = "- first\nlazy line\n# heading\n- second\n";
    assert_eq!(kinds(src).len(), 2);
    assert!(kinds(src).iter().all(|(k, _)| *k == BlockKind::Bullet));
}

#[test]
fn a_fence_inside_a_bullet_keeps_its_unindented_lines() {
    let src = "- a\n  ```\n  x\n\ncolumn zero inside the fence\n  ```\n- b\n";
    let k = kinds(src);
    assert_eq!(k.len(), 2, "{k:?}");
    assert!(k.iter().all(|(k, _)| *k == BlockKind::Bullet));
}

#[test]
fn bullets_after_text_are_never_children_of_earlier_bullets() {
    let src = "- a\n  - child\n\ntext\n\n  - indented bullet after text stays in the text\n- b\n";
    let p = parse_page(src);
    assert_eq!(p.blocks.len(), 3);
    assert_eq!(p.blocks[0].children.len(), 1);
    assert_eq!(p.blocks[1].kind, BlockKind::Text);
    assert!(
        p.blocks[1]
            .edit_text(src)
            .contains("  - indented bullet after text")
    );
    assert_eq!(p.blocks[2].kind, BlockKind::Bullet);
}

fn run(src: &str, op: BlockOp) -> String {
    let p = parse_page(src);
    apply(src, &p, &op).unwrap().new_source
}

#[test]
fn editing_a_text_block_writes_it_back_as_typed() {
    let src = "- a\n\n## Old\n\n- b\n";
    let out = run(
        src,
        BlockOp::UpdateText {
            block: BlockRef::at(&[1]),
            text: "## New\n    indented stays".into(),
        },
    );
    assert_eq!(out, "- a\n\n## New\n    indented stays\n\n- b\n");
    // and the same text read back is what was typed
    let p = parse_page(&out);
    assert_eq!(p.blocks[1].edit_text(&out), "## New\n    indented stays");
}

#[test]
fn text_blocks_cannot_be_nested_and_nothing_nests_under_them() {
    let src = "- a\n\ntext\n\n- b\n";
    let p = parse_page(src);
    for op in [
        BlockOp::Indent {
            block: BlockRef::at(&[1]),
        },
        BlockOp::Indent {
            block: BlockRef::at(&[2]),
        },
        BlockOp::Move {
            block: BlockRef::at(&[1]),
            new_parent: vec![0],
            index: 0,
        },
        BlockOp::Move {
            block: BlockRef::at(&[2]),
            new_parent: vec![1],
            index: 0,
        },
        BlockOp::Insert {
            parent: vec![1],
            index: 0,
            text: "x".into(),
        },
    ] {
        assert!(apply(src, &p, &op).is_err(), "{op:?}");
    }
}

#[test]
fn a_bullet_put_before_text_gets_a_blank_line_so_it_does_not_swallow_the_text() {
    let src = "- a\n\n## H\n";
    // inserted before the text
    let out = run(
        src,
        BlockOp::Insert {
            parent: vec![],
            index: 1,
            text: "new".into(),
        },
    );
    assert_eq!(out, "- a\n\n- new\n\n## H\n");
    assert_eq!(kinds(&out).len(), 3);
    // moved before the text
    let src = "- a\n- b\n\n## H\n";
    let out = run(
        src,
        BlockOp::Move {
            block: BlockRef::at(&[0]),
            new_parent: vec![],
            index: 1,
        },
    );
    let k = kinds(&out);
    assert_eq!(k.len(), 3, "{out:?}");
    assert_eq!(k[2].0, BlockKind::Text, "{out:?}");
}

#[test]
fn text_moved_below_a_bullet_gets_a_blank_line_before_it() {
    let src = "## H\n\n- a\n- b\n";
    let p = parse_page(src);
    // the heading is the preamble here; make it a text block by putting a bullet first
    let _ = p;
    let src = "- z\n\n## H\n\n- a\n- b\n";
    let out = run(
        src,
        BlockOp::Move {
            block: BlockRef::at(&[1]),
            new_parent: vec![],
            index: 3,
        },
    );
    let k = kinds(&out);
    assert_eq!(k.len(), 4, "{out:?}");
    assert_eq!(k[3].0, BlockKind::Text, "{out:?}");
    assert!(out.ends_with("- b\n\n## H\n"), "{out:?}");
}

#[test]
fn deleting_a_text_block_leaves_its_neighbours_alone() {
    let src = "- a\n\n## H\n\n- b\n";
    assert_eq!(
        run(
            src,
            BlockOp::Delete {
                block: BlockRef::at(&[1])
            }
        ),
        "- a\n\n\n- b\n"
    );
}
