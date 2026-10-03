//! Parser tests over all fixture files: snapshots, span invariants, coverage, round trip.
mod common;

use notatnik_core::{Block, BlockKind, Page, parse_page};

fn check_block(src: &str, b: &Block, parent: Option<&Block>, path: &str) {
    let ctx = format!("{path} @{}", b.span.start);
    assert!(b.span.end <= src.len(), "{ctx}: span out of bounds");
    assert!(
        src.is_char_boundary(b.span.start) && src.is_char_boundary(b.span.end),
        "{ctx}: char boundary"
    );
    if b.kind == BlockKind::Bullet {
        let t = b.span.slice(src).trim_start_matches([' ', '\t']);
        assert!(
            t.starts_with(['-', '*', '+']),
            "{ctx}: span must start with its bullet, got {t:?}"
        );
    }
    assert!(b.span.contains_span(&b.own_span), "{ctx}");
    if let Some(p) = parent {
        assert!(p.span.contains_span(&b.span), "{ctx}: child outside parent");
        assert!(
            p.own_span.end <= b.span.start,
            "{ctx}: child before parent's own lines end"
        );
    }
    let mut prev_end = b.own_span.end;
    for c in &b.children {
        assert!(
            c.span.start >= prev_end,
            "{ctx}: siblings overlap or out of order"
        );
        prev_end = c.span.end;
        check_block(src, c, Some(b), path);
    }
}

fn check_invariants(src: &str, page: &Page, path: &str) {
    let mut prev = 0;
    for s in page.covered_spans() {
        assert!(
            s.start >= prev,
            "{path}: top-level spans overlap or are out of order"
        );
        prev = s.end;
    }
    if let Some(p) = &page.preamble {
        check_block(src, p, None, path);
    }
    for b in &page.blocks {
        check_block(src, b, None, path);
    }
    let unc = page.uncovered_non_blank(src);
    assert!(
        unc.is_empty(),
        "{path}: uncovered text {:?}",
        unc.iter().map(|s| s.slice(src)).collect::<Vec<_>>()
    );
}

#[test]
fn snapshots_for_every_fixture() {
    let files = common::fixture_files();
    assert!(
        files.len() > 30,
        "expected many fixture files, got {}",
        files.len()
    );
    for (path, src) in &files {
        let page = parse_page(src);
        insta::with_settings!({ snapshot_path => "snapshots/fixtures", prepend_module_to_snapshot => false }, {
            insta::assert_json_snapshot!(path.replace('/', "__"), page);
        });
    }
}

#[test]
fn span_invariants_and_coverage_on_all_fixtures() {
    for (path, src) in common::fixture_files() {
        let page = parse_page(&src);
        check_invariants(&src, &page, &path);
    }
}

#[test]
fn no_op_rebuild_is_byte_identical() {
    for (path, src) in common::fixture_files() {
        assert_eq!(parse_page(&src).rebuild(&src), src, "{path}");
    }
}

fn page_of(rel: &str) -> (String, Page) {
    let src = common::fixture_files()
        .into_iter()
        .find(|(p, _)| p == rel)
        .unwrap_or_else(|| panic!("no fixture {rel}"))
        .1;
    let page = parse_page(&src);
    (src, page)
}

fn texts(src: &str, blocks: &[Block]) -> Vec<String> {
    blocks
        .iter()
        .map(|b| b.first_line.slice(src).to_string())
        .collect()
}

#[test]
fn crlf_and_bom_and_newlines() {
    let (_, p) = page_of("edge-cases/pages/crlf.md");
    assert_eq!(p.style.eol, notatnik_core::Eol::Crlf);
    assert!(p.diagnostics.is_empty());
    let (src, p) = page_of("edge-cases/pages/crlf.md");
    assert_eq!(p.blocks[0].first_line.slice(&src), "Parent ^abc123");
    assert_eq!(p.blocks[0].properties[0].raw, "value");
    assert_eq!(p.blocks[0].block_id.as_ref().unwrap().id, "abc123");

    let (src, p) = page_of("edge-cases/pages/bom.md");
    assert!(p.style.bom);
    assert_eq!(
        p.frontmatter.as_ref().unwrap().title.as_deref(),
        Some("With BOM")
    );
    assert_eq!(p.blocks[0].first_line.slice(&src), "First block");

    let (_, p) = page_of("edge-cases/pages/no-trailing-newline.md");
    assert!(!p.style.final_newline);
    assert_eq!(p.blocks[0].children.len(), 1);

    let (_, p) = page_of("edge-cases/pages/empty.md");
    assert!(p.blocks.is_empty() && p.preamble.is_none() && p.diagnostics.is_empty());

    let (_, p) = page_of("edge-cases/pages/only-frontmatter.md");
    assert!(p.blocks.is_empty() && p.frontmatter.as_ref().unwrap().tags == ["a"]);
}

#[test]
fn indentation_styles() {
    use notatnik_core::IndentStyle;
    let (_, p) = page_of("edge-cases/pages/tabs.md");
    assert_eq!(p.style.indent, IndentStyle::Tabs);
    assert_eq!(p.blocks[0].children.len(), 2);
    assert_eq!(p.blocks[0].children[0].children.len(), 1);
    let (_, p) = page_of("edge-cases/pages/four-space.md");
    assert_eq!(p.style.indent, IndentStyle::Spaces(4));
    assert_eq!(p.blocks[0].children.len(), 2);
    // Mixed: a tab counts as width 4, so the tab line and the four-space line are both children
    // of the two-space block (width 2) and siblings of each other.
    let (_, p) = page_of("edge-cases/pages/mixed-indent.md");
    assert_eq!(p.blocks.len(), 1);
    assert_eq!(p.blocks[0].children.len(), 1);
    assert_eq!(p.blocks[0].children[0].children.len(), 2);
    // Jumps: child at 6 under parent at 0 is simply a child; deeper jump is a grandchild level by depth.
    let (src, p) = page_of("edge-cases/pages/indent-jump.md");
    assert_eq!(p.blocks.len(), 2);
    assert_eq!(
        texts(&src, &p.blocks[0].children),
        ["Jumped by six", "Back to two"]
    );
    assert_eq!(p.blocks[0].children[1].children.len(), 1);
    assert_eq!(p.blocks[0].children[1].children[0].level, 2);
}

#[test]
fn bullet_kinds() {
    let (src, p) = page_of("edge-cases/pages/bullets.md");
    // * Star (with + child), - Dash (which swallows the numbered line), empty, empty, key-only, empty-with-props
    assert_eq!(p.blocks.len(), 6, "{:?}", texts(&src, &p.blocks));
    assert_eq!(p.blocks[0].children.len(), 1);
    assert!(
        p.blocks[1].rest_span.is_some(),
        "numbered line is continuation text of the dash block"
    );
    assert_eq!(p.blocks[4].properties.len(), 1);
    assert_eq!(p.blocks[5].first_line.slice(&src), "");
    assert_eq!(p.blocks[5].properties[0].key, "status");
}

#[test]
fn code_fences_are_opaque() {
    let (src, p) = page_of("edge-cases/pages/fences.md");
    assert!(p.preamble.is_none());
    let tops = texts(&src, &p.blocks);
    assert_eq!(tops.len(), 7, "{tops:?}");
    assert!(p.blocks.iter().all(|b| b.children.is_empty()));
    for b in &p.blocks[..3] {
        assert!(
            b.properties.is_empty() && b.refs.is_empty() && b.task.is_none(),
            "{:?}",
            b.first_line.slice(&src)
        );
    }
    assert_eq!(p.blocks[3].refs[0].target, "real");
    // inline triple backticks on the first line are not a fence
    assert_eq!(p.blocks[4].refs[0].target, "inline");
    assert!(p.diagnostics.iter().any(|d| d.code == "unclosed-fence"));
}

#[test]
fn frontmatter_edge_cases() {
    let (src, p) = page_of("edge-cases/pages/bad-frontmatter.md");
    let fm = p.frontmatter.as_ref().unwrap();
    assert!(!fm.valid);
    assert!(fm.span.slice(&src).starts_with("---\ntitle: [unclosed"));
    let d = p
        .diagnostics
        .iter()
        .find(|d| d.code == "invalid-frontmatter")
        .expect("diagnostic");
    assert!(fm.span.contains_span(&d.span));
    assert_eq!(p.blocks.len(), 1);

    let (src, p) = page_of("edge-cases/pages/hr-later.md");
    assert!(p.frontmatter.is_none());
    assert_eq!(
        p.preamble.as_ref().unwrap().first_line.slice(&src),
        "Some preamble text."
    );
    // the `---` after the blank line belongs to the preamble text
    assert!(
        p.preamble
            .as_ref()
            .unwrap()
            .span
            .slice(&src)
            .contains("---")
    );

    let (_, p) = page_of("edge-cases/pages/unclosed-frontmatter.md");
    assert!(p.frontmatter.is_none());
}

#[test]
fn text_between_bullets_and_headings() {
    let (src, p) = page_of("edge-cases/pages/text-between.md");
    let pre = p.preamble.as_ref().unwrap();
    assert!(pre.span.slice(&src).contains("Plain paragraph"));
    assert_eq!(pre.refs[0].target, "a link");
    assert_eq!(p.blocks.len(), 2);
    assert!(
        p.blocks[0]
            .span
            .slice(&src)
            .contains("# Heading between bullets")
    );
}

#[test]
fn block_ids() {
    let (src, p) = page_of("edge-cases/pages/block-ids.md");
    let ids: Vec<Option<&str>> = p
        .blocks
        .iter()
        .map(|b| b.block_id.as_ref().map(|i| i.id.as_str()))
        .collect();
    assert_eq!(
        ids,
        [
            Some("abc123"),
            None,
            None,
            None,
            Some("def456"),
            Some("ghi789"),
            Some("abc123")
        ]
    );
    assert_eq!(
        p.blocks[0].block_id.as_ref().unwrap().span.slice(&src),
        " ^abc123"
    );
    assert!(p.diagnostics.iter().any(|d| d.code == "duplicate-block-id"));
}

#[test]
fn deep_nesting_and_long_lines() {
    let (_, p) = page_of("edge-cases/pages/deep-nesting.md");
    let mut depth = 0;
    let mut b = &p.blocks[0];
    while let Some(c) = b.children.first() {
        depth += 1;
        b = c;
    }
    assert_eq!(depth, 49);
    let long = format!("- {} [[x]]\n- next\n", "word ".repeat(20_000));
    let p = parse_page(&long);
    assert_eq!(p.blocks.len(), 2);
    assert_eq!(p.blocks[0].refs[0].target, "x");
    check_invariants(&long, &p, "long");
}

#[test]
fn very_deep_nesting_is_flattened_not_crashing() {
    let src: String = (0..400)
        .map(|i| format!("{}- x\n", " ".repeat(i)))
        .collect();
    let p = parse_page(&src);
    assert!(p.diagnostics.iter().any(|d| d.code == "too-deep"));
    check_invariants(&src, &p, "deep");
}

#[test]
fn inline_edge_cases_on_fixtures() {
    let (src, p) = page_of("edge-cases/pages/hash-cases.md");
    let tags =
        |i: usize| -> Vec<String> { p.blocks[i].refs.iter().map(|r| r.target.clone()).collect() };
    assert!(
        tags(0).is_empty()
            && tags(1).is_empty()
            && tags(2).is_empty()
            && tags(3).is_empty()
            && tags(4).is_empty()
    );
    assert_eq!(
        tags(5),
        [
            "tag",
            "paren",
            "tag",
            "123",
            "tag_name",
            "tagName",
            "tag-name",
            "zażółć",
            "a/b",
            "multi word"
        ]
    );
    assert_eq!(tags(6), ["tag"]);
    let _ = src;
    let (_, p) = page_of("edge-cases/pages/brackets.md");
    assert!(p.blocks[0].refs.is_empty());
    assert_eq!(
        p.blocks[1]
            .refs
            .iter()
            .map(|r| r.target.as_str())
            .collect::<Vec<_>>(),
        ["b"]
    );
    assert!(p.blocks[2].refs.is_empty());
    assert!(p.blocks[3].refs.is_empty());
    assert_eq!(
        p.diagnostics
            .iter()
            .filter(|d| d.code == "relative-link")
            .count(),
        2
    );
    assert_eq!(
        p.diagnostics
            .iter()
            .filter(|d| d.code == "empty-link")
            .count(),
        1
    );
    assert_eq!(p.blocks[4].refs[0].target, "work/alpha");
    assert_eq!(p.blocks[5].refs.len(), 2);
}

#[test]
fn html_is_text() {
    let (_, p) = page_of("edge-cases/pages/html.md");
    assert!(p.diagnostics.is_empty());
    assert_eq!(p.blocks[0].refs.len(), 1);
}

mod prop {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(10_000))]
        #[test]
        fn never_panics_and_spans_are_valid(bytes in proptest::collection::vec(any::<u8>(), 0..300)) {
            let src = String::from_utf8_lossy(&bytes).into_owned();
            let page = parse_page(&src);
            check_invariants(&src, &page, "prop");
            prop_assert_eq!(page.rebuild(&src), src.clone());
            for d in &page.diagnostics {
                prop_assert!(d.span.end <= src.len() && src.is_char_boundary(d.span.start) && src.is_char_boundary(d.span.end));
            }
        }

        #[test]
        fn structured_text_never_panics(src in "(?s)([-*+ \t]{0,6}(TODO |DONE |#|\\[\\[|\\]\\]|\\(\\(|\\)\\)|`|~~~|```|key:: |\\^|abc123|\n|\r\n|---|[a-z ]{0,5})){0,40}") {
            let page = parse_page(&src);
            check_invariants(&src, &page, "prop2");
            prop_assert_eq!(page.rebuild(&src), src);
        }
    }
}
