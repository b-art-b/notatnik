//! P5.1: three-way merge of pages at block level.

use notatnik_core::{Choice, ConflictKind, merge, merge_with, parse_page};
use proptest::prelude::*;

const BASE: &str = "- one\n- two\n  - two a\n  - two b\n- three\n";

fn clean(base: &str, ours: &str, theirs: &str) -> String {
    let r = merge(base, ours, theirs).unwrap();
    assert!(r.is_clean(), "conflicts: {:?}", r.conflicts);
    // a clean merge reparses with no diagnostics that the inputs did not have
    let n = |s: &str| parse_page(s).diagnostics.len();
    assert!(n(&r.merged_text) <= n(ours).max(n(theirs)).max(n(base)));
    r.merged_text
}

#[test]
fn different_blocks_edited_merge_cleanly() {
    let ours = BASE.replace("- one", "- one!");
    let theirs = BASE.replace("- three", "- three!");
    assert_eq!(
        clean(BASE, &ours, &theirs),
        "- one!\n- two\n  - two a\n  - two b\n- three!\n"
    );
}

#[test]
fn the_same_edit_on_both_sides_is_taken_once() {
    let both = BASE.replace("- two\n", "- 2\n");
    assert_eq!(clean(BASE, &both, &both), both);
}

#[test]
fn the_same_block_edited_differently_conflicts_and_keeps_ours_by_default() {
    let ours = BASE.replace("- one", "- ours");
    let theirs = BASE.replace("- one", "- theirs");
    let r = merge(BASE, &ours, &theirs).unwrap();
    assert_eq!(r.conflicts.len(), 1);
    let c = &r.conflicts[0];
    assert_eq!(c.kind, ConflictKind::Edit);
    assert_eq!(c.base.as_deref(), Some("- one\n"));
    assert_eq!(c.ours.as_deref(), Some("- ours\n"));
    assert_eq!(c.theirs.as_deref(), Some("- theirs\n"));
    assert_eq!(r.merged_text, ours);

    let pick = |c: Choice| merge_with(BASE, &ours, &theirs, &[c]).unwrap().merged_text;
    assert_eq!(pick(Choice::Theirs), theirs);
    assert_eq!(pick(Choice::Ours), ours);
    assert_eq!(
        pick(Choice::Both),
        "- ours\n- theirs\n- two\n  - two a\n  - two b\n- three\n"
    );
    assert_eq!(
        pick(Choice::Text("- mine\n".into())),
        BASE.replace("- one", "- mine")
    );
}

#[test]
fn a_deleted_block_that_the_other_side_edited_is_kept_and_reported() {
    let ours = "- one\n- three\n"; // deleted "two" with its children
    let theirs = BASE.replace("two b", "two B");
    let r = merge(BASE, ours, &theirs).unwrap();
    assert_eq!(r.conflicts.len(), 1);
    assert_eq!(r.conflicts[0].kind, ConflictKind::EditDelete);
    assert_eq!(r.conflicts[0].ours, None);
    assert_eq!(r.merged_text, theirs);
    let gone = merge_with(BASE, ours, &theirs, &[Choice::Ours]).unwrap();
    assert_eq!(gone.merged_text, ours);
}

#[test]
fn a_deleted_block_that_the_other_side_left_alone_is_deleted() {
    let ours = "- one\n- three\n";
    assert_eq!(clean(BASE, ours, BASE), ours);
    assert_eq!(clean(BASE, BASE, ours), ours);
}

#[test]
fn blocks_added_at_the_same_place_are_both_kept_ours_first() {
    let ours = BASE.replace("- one\n", "- one\n- from ours\n");
    let theirs = BASE.replace("- one\n", "- one\n- from theirs\n");
    assert_eq!(
        clean(BASE, &ours, &theirs),
        "- one\n- from ours\n- from theirs\n- two\n  - two a\n  - two b\n- three\n"
    );
}

#[test]
fn the_same_block_added_on_both_sides_is_kept_once() {
    let both = BASE.replace("- one\n", "- one\n- new\n");
    assert_eq!(clean(BASE, &both, &both), both);
}

#[test]
fn blocks_added_in_different_places_land_where_they_were_added() {
    let ours = format!("- top\n{BASE}");
    let theirs = format!("{BASE}- end\n");
    assert_eq!(clean(BASE, &ours, &theirs), format!("- top\n{BASE}- end\n"));
}

#[test]
fn children_are_merged_below_a_block_both_sides_changed() {
    let ours = BASE.replace("- two\n", "- two!\n");
    let theirs = BASE.replace("two b", "two B");
    assert_eq!(
        clean(BASE, &ours, &theirs),
        "- one\n- two!\n  - two a\n  - two B\n- three\n"
    );
}

#[test]
fn properties_are_merged_per_key() {
    let base = "- task\n  a:: 1\n  b:: 2\n";
    let ours = "- task\n  a:: 10\n  b:: 2\n";
    let theirs = "- task\n  a:: 1\n  b:: 20\n  c:: 3\n";
    assert_eq!(
        clean(base, ours, theirs),
        "- task\n  a:: 10\n  b:: 20\n  c:: 3\n"
    );
    // the same key changed differently is a conflict
    let theirs = "- task\n  a:: 11\n  b:: 2\n";
    let r = merge(base, ours, theirs).unwrap();
    assert_eq!(r.conflicts.len(), 1);
}

#[test]
fn a_property_deleted_on_one_side_and_changed_on_the_other_conflicts() {
    let base = "- t\n  a:: 1\n  b:: 2\n";
    let r = merge(base, "- t\n  b:: 2\n", "- t\n  a:: 9\n  b:: 2\n").unwrap();
    assert_eq!(r.conflicts.len(), 1);
}

#[test]
fn continuation_lines_are_merged_by_line() {
    let base = "- x\n  l1\n  l2\n  l3\n  l4\n";
    let ours = "- x\n  l1!\n  l2\n  l3\n  l4\n";
    let theirs = "- x\n  l1\n  l2\n  l3\n  l4!\n";
    assert_eq!(clean(base, ours, theirs), "- x\n  l1!\n  l2\n  l3\n  l4!\n");
    let theirs = "- x\n  l1?\n  l2\n  l3\n  l4\n";
    assert_eq!(merge(base, ours, theirs).unwrap().conflicts.len(), 1);
}

#[test]
fn block_ids_keep_blocks_together_when_text_and_place_change() {
    let base = "- alpha ^aaaaaa\n- beta ^bbbbbb\n- gamma ^cccccc\n";
    let ours = "- gamma ^cccccc\n- alpha ^aaaaaa\n- beta ^bbbbbb\n"; // moved
    let theirs = "- alpha edited ^aaaaaa\n- beta ^bbbbbb\n- gamma ^cccccc\n";
    assert_eq!(
        clean(base, ours, theirs),
        "- gamma ^cccccc\n- alpha edited ^aaaaaa\n- beta ^bbbbbb\n"
    );
}

#[test]
fn a_block_that_was_only_reordered_keeps_that_order() {
    let ours = "- three\n- one\n- two\n  - two a\n  - two b\n";
    let theirs = BASE.replace("- one", "- one!");
    assert_eq!(
        clean(BASE, ours, &theirs),
        "- three\n- one!\n- two\n  - two a\n  - two b\n"
    );
}

#[test]
fn blank_lines_between_blocks_survive() {
    let base = "- a\n\n- b\n\n- c\n";
    let ours = "- a!\n\n- b\n\n- c\n";
    let theirs = "- a\n\n- b\n\n- c!\n";
    assert_eq!(clean(base, ours, theirs), "- a!\n\n- b\n\n- c!\n");
}

#[test]
fn front_matter_changed_on_one_side_is_taken() {
    let base = "---\ntitle: X\n---\n- a\n";
    let ours = "---\ntitle: X\nicon: a\n---\n- a\n";
    let theirs = "---\ntitle: X\n---\n- a\n- b\n";
    assert_eq!(
        clean(base, ours, theirs),
        "---\ntitle: X\nicon: a\n---\n- a\n- b\n"
    );
}

#[test]
fn front_matter_changed_on_both_sides_is_a_text_conflict() {
    let base = "---\ntitle: X\n---\n- a\n";
    let ours = "---\ntitle: Y\n---\n- a\n";
    let theirs = "---\ntitle: Z\n---\n- a\n";
    let r = merge(base, ours, theirs).unwrap();
    assert_eq!(r.conflicts.len(), 1);
    assert_eq!(r.conflicts[0].kind, ConflictKind::Text);
    assert_eq!(
        merge_with(base, ours, theirs, &[Choice::Theirs])
            .unwrap()
            .merged_text,
        theirs
    );
}

#[test]
fn a_file_without_a_final_newline_gets_one_when_blocks_follow() {
    let base = "- a\n- b";
    let ours = "- a\n- b";
    let theirs = "- a\n- b\n- c\n";
    assert_eq!(clean(base, ours, theirs), "- a\n- b\n- c\n");
    let ours = "- a\n- b\n- c";
    assert_eq!(clean("- a\n- b", ours, "- a\n- b"), ours);
}

#[test]
fn text_before_the_first_bullet_is_merged_like_a_block() {
    let base = "intro\n- a\n";
    let ours = "intro!\n- a\n";
    let theirs = "intro\n- a\n- b\n";
    assert_eq!(clean(base, ours, theirs), "intro!\n- a\n- b\n");
}

// ---- properties of the merge ----

fn word() -> impl Strategy<Value = String> {
    prop::sample::select(vec![
        "alpha", "beta", "gamma", "delta", "eps", "zeta", "eta", "theta",
    ])
    .prop_map(String::from)
}

/// A page: nested bullets (depth up to 3), sometimes with a property or a continuation line.
fn page() -> impl Strategy<Value = String> {
    prop::collection::vec((0usize..3, word(), word(), 0u8..4), 0..12).prop_map(|rows| {
        let mut out = String::new();
        let mut depth = 0usize;
        for (d, a, b, extra) in rows {
            depth = d.min(depth + 1);
            let ind = "  ".repeat(depth);
            out.push_str(&format!("{ind}- {a} {b}\n"));
            match extra {
                1 => out.push_str(&format!("{ind}  k:: {a}\n")),
                2 => out.push_str(&format!("{ind}  more {b}\n")),
                _ => {}
            }
        }
        out
    })
}

proptest! {
    /// Nothing changed on one side: the result is the other side, byte for byte.
    #[test]
    fn merging_with_the_base_gives_the_other_side(base in page(), x in page()) {
        prop_assert_eq!(merge(&base, &x, &base).unwrap().merged_text, x.clone());
        prop_assert_eq!(merge(&base, &base, &x).unwrap().merged_text, x);
    }

    /// Merging a page with itself changes nothing.
    #[test]
    fn identical_sides_give_the_same_page(base in page(), x in page()) {
        let r = merge(&base, &x, &x).unwrap();
        prop_assert!(r.is_clean());
        prop_assert_eq!(r.merged_text, x);
    }

    /// Whatever the sides are, a merge ends and its text parses as a page.
    #[test]
    fn any_three_pages_merge_without_losing_the_structure(b in page(), o in page(), t in page()) {
        let r = merge(&b, &o, &t).unwrap();
        let blocks = |s: &str| parse_page(s).all_blocks().len();
        prop_assert!(blocks(&r.merged_text) >= blocks(&o).min(blocks(&t)).min(blocks(&b)) / 2);
        prop_assert!(parse_page(&r.merged_text).diagnostics.len() <= 3);
    }
}
