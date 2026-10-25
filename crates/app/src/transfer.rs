//! The text work of moving notes from one graph to another (MG-2, MG-3): finding the pictures a text mentions,
//! pointing them at their new place, taking a few blocks out of a page, giving block IDs that clash a new name.
//! Pure functions; the graphs themselves are touched in `state/transfer.rs`.

use std::collections::HashSet;

use notatnik_core::{
    Block, BlockOp, Page, ParseOptions, RefKind, Span, TextPatch, apply_patches, ids,
    parse_page_with, replace_block_id,
};

/// A file below `assets/` that a text points at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetRef {
    /// The whole reference in the text, `../../assets/2026/10/a.png` (bytes).
    pub range: Span,
    /// How many `../` come first.
    pub ups: usize,
    /// The part from `assets/` on, as written (it may be percent-encoded).
    pub path: String,
}

/// Every reference to a file below `assets/` in `text`: in a link, an image or HTML. A web address that happens to
/// contain `/assets/` is not one.
pub fn find_asset_refs(text: &str) -> Vec<AssetRef> {
    const KEY: &str = "assets/";
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(p) = text.get(from..).and_then(|t| t.find(KEY)) {
        let at = from + p;
        from = at + KEY.len();
        let mut start = at;
        let mut ups = 0;
        while start >= 3 && text.get(start - 3..start) == Some("../") {
            start -= 3;
            ups += 1;
        }
        let ok = match text.get(..start).and_then(|t| t.chars().next_back()) {
            None => true,
            Some(c) => c.is_whitespace() || matches!(c, '(' | '"' | '\'' | '<' | '[' | '='),
        };
        if !ok {
            continue;
        }
        let rest = text.get(at..).unwrap_or("");
        let len = rest
            .find(|c: char| c.is_whitespace() || matches!(c, ')' | '"' | '\'' | '>' | ']'))
            .unwrap_or(rest.len());
        if len <= KEY.len() {
            continue;
        }
        out.push(AssetRef {
            range: Span::new(start, at + len),
            ups,
            path: rest.get(..len).unwrap_or("").to_string(),
        });
        from = at + len;
    }
    out
}

/// Decodes `%XX` escapes.
pub fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        let hex = |c: u8| (c as char).to_digit(16);
        if b[i] == b'%' && i + 2 < b.len() && hex(b[i + 1]).is_some() && hex(b[i + 2]).is_some() {
            out.push((hex(b[i + 1]).unwrap_or(0) * 16 + hex(b[i + 2]).unwrap_or(0)) as u8);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// `a/b.png` + `1f2e3d4c` → `a/b-1f2e3d4c.png`: the name a picture gets in the target when a different one is there.
pub fn renamed_asset(path: &str, hash8: &str) -> String {
    let name_at = path.rfind('/').map_or(0, |i| i + 1);
    match path.get(name_at..).and_then(|n| n.rfind('.')) {
        Some(dot) if dot > 0 => {
            let at = name_at + dot;
            format!("{}-{hash8}{}", &path[..at], &path[at..])
        }
        _ => format!("{path}-{hash8}"),
    }
}

/// Replaces each reference for which `f` answers with the new text.
pub fn rewrite_asset_refs(
    text: &str,
    refs: &[AssetRef],
    f: impl Fn(&AssetRef) -> Option<String>,
) -> String {
    let patches: Vec<TextPatch> = refs
        .iter()
        .filter_map(|r| {
            let new_text = f(r)?;
            Some(TextPatch {
                range: r.range,
                old_text: r.range.slice(text).to_string(),
                new_text,
            })
        })
        .collect();
    apply_patches(text, &patches)
}

/// The text of the given blocks (each with its children) as a small outline of their own: the lines as they are in
/// the page, moved to the left edge.
pub fn outline_text(source: &str, blocks: &[&Block]) -> String {
    let mut out = String::new();
    for b in blocks {
        let strip = b.indent_span.len();
        for line in b.span.slice(source).split_inclusive('\n') {
            let n = line
                .bytes()
                .take(strip)
                .take_while(|c| *c == b' ' || *c == b'\t')
                .count();
            out.push_str(line.get(n..).unwrap_or(line));
        }
        if !out.ends_with('\n') {
            out.push('\n');
        }
    }
    out
}

/// Every block of the tree, parents first, with its path.
pub fn walk<'a>(blocks: &'a [Block], f: &mut impl FnMut(&[usize], &'a Block)) {
    fn go<'a>(blocks: &'a [Block], path: &mut Vec<usize>, f: &mut impl FnMut(&[usize], &'a Block)) {
        for (i, b) in blocks.iter().enumerate() {
            path.push(i);
            f(path, b);
            go(&b.children, path, f);
            path.pop();
        }
    }
    go(blocks, &mut Vec::new(), f);
}

/// The block IDs defined in `text`.
pub fn defined_ids(text: &str, opts: &ParseOptions) -> HashSet<String> {
    let page = parse_page_with(text, opts);
    let mut out = HashSet::new();
    walk(&page.blocks, &mut |_, b| {
        if let Some(id) = &b.block_id {
            out.insert(id.id.clone());
        }
    });
    out
}

/// The IDs that `((…))` references and `!((…))` embeds in `text` point at.
pub fn referenced_ids(text: &str, opts: &ParseOptions) -> HashSet<String> {
    let page = parse_page_with(text, opts);
    let mut out = HashSet::new();
    walk(&page.blocks, &mut |_, b| {
        for r in &b.refs {
            if matches!(r.kind, RefKind::Block | RefKind::BlockEmbed) {
                out.insert(r.target.clone());
            }
        }
    });
    out
}

/// Gives every block whose ID is in `taken` a new one, and follows it in the `((…))` references of the text. Returns
/// the new text and the `(old, new)` pairs. The IDs the text keeps are added to `taken`.
pub fn rekey_ids(
    text: &str,
    opts: &ParseOptions,
    taken: &mut HashSet<String>,
    rng: &mut impl FnMut() -> u64,
) -> (String, Vec<(String, String)>) {
    let mut text = text.to_string();
    let mut changed = Vec::new();
    loop {
        let page = parse_page_with(&text, opts);
        let mut clash: Option<(Vec<usize>, String)> = None;
        walk(&page.blocks, &mut |path, b| {
            if clash.is_none() {
                if let Some(id) = &b.block_id {
                    if taken.contains(&id.id) {
                        clash = Some((path.to_vec(), id.id.clone()));
                    }
                }
            }
        });
        let Some((path, old)) = clash else {
            break;
        };
        let mut avoid = taken.clone();
        avoid.extend(defined_ids(&text, opts));
        let new = ids::generate_block_id(&avoid, rng);
        match replace_block_id(&text, &page, &path, &new) {
            Ok(r) => text = r.new_source,
            Err(_) => break,
        }
        text = text.replace(&format!("(({old}))"), &format!("(({new}))"));
        changed.push((old, new));
    }
    taken.extend(defined_ids(&text, opts));
    (text, changed)
}

/// Inserts that add the top-level blocks of `outline` (and what is below them) as the last blocks of a page that has
/// `existing` top-level blocks. Each block keeps its ID, if it has one.
pub fn ops_for_outline(outline: &str, existing: usize, opts: &ParseOptions) -> Vec<BlockOp> {
    let page: Page = parse_page_with(outline, opts);
    let mut ops = Vec::new();
    // parents come before their children and the children in order, so the path of each new block is known
    walk(&page.blocks, &mut |path, b| {
        let mut text = b.edit_text(outline);
        if let Some(id) = &b.block_id {
            let end = text.find('\n').unwrap_or(text.len());
            text.insert_str(end, &format!(" ^{}", id.id));
        }
        let mut parent: Vec<usize> = path.get(..path.len() - 1).unwrap_or(&[]).to_vec();
        let mut index = *path.last().unwrap_or(&0);
        if parent.is_empty() {
            index += existing;
        } else {
            parent[0] += existing;
        }
        ops.push(BlockOp::Insert {
            parent,
            index,
            text,
        });
    });
    ops
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asset_references_are_found_with_their_prefix() {
        let t = "![](../../assets/2026/10/a.png) and [f](../assets/x%20y.pdf \"t\") and https://x.org/assets/b.png";
        let r = find_asset_refs(t);
        assert_eq!(r.len(), 2);
        assert_eq!(r[0].ups, 2);
        assert_eq!(r[0].path, "assets/2026/10/a.png");
        assert_eq!(r[0].range.slice(t), "../../assets/2026/10/a.png");
        assert_eq!(r[1].ups, 1);
        assert_eq!(r[1].path, "assets/x%20y.pdf");
        assert_eq!(percent_decode(&r[1].path), "assets/x y.pdf");
    }

    #[test]
    fn a_renamed_picture_keeps_its_folder_and_extension() {
        assert_eq!(
            renamed_asset("assets/2026/10/a.png", "1f2e3d4c"),
            "assets/2026/10/a-1f2e3d4c.png"
        );
        assert_eq!(renamed_asset("assets/x", "ab"), "assets/x-ab");
        assert_eq!(renamed_asset("assets/.hid", "ab"), "assets/.hid-ab");
    }

    #[test]
    fn references_are_rewritten_in_place() {
        let t = "a ![](../assets/a.png) b ![](../assets/c.png)";
        let r = find_asset_refs(t);
        let out = rewrite_asset_refs(t, &r, |r| {
            (r.path == "assets/a.png").then(|| format!("{}assets/z.png", "../".repeat(r.ups + 1)))
        });
        assert_eq!(out, "a ![](../../assets/z.png) b ![](../assets/c.png)");
    }

    #[test]
    fn blocks_move_to_the_left_edge_with_their_children() {
        let src = "- a\n  - b ^abc123\n    - c\n- d\n";
        let opts = ParseOptions::default();
        let page = parse_page_with(src, &opts);
        let b = &page.blocks[0].children[0];
        let o = outline_text(src, &[b]);
        assert_eq!(o, "- b ^abc123\n  - c\n");
        let ops = ops_for_outline(&o, 2, &opts);
        assert_eq!(ops.len(), 2);
        assert_eq!(
            ops[0],
            BlockOp::Insert {
                parent: vec![],
                index: 2,
                text: "b ^abc123".into()
            }
        );
        assert_eq!(
            ops[1],
            BlockOp::Insert {
                parent: vec![2],
                index: 0,
                text: "c".into()
            }
        );
    }

    #[test]
    fn a_clashing_id_gets_a_new_one_and_the_references_follow() {
        let src = "- a ^abc123\n- see ((abc123))\n- keep ^zzz999\n";
        let opts = ParseOptions::default();
        let mut taken: HashSet<String> = ["abc123".to_string()].into();
        let mut n = 0u64;
        let mut rng = || {
            n += 7;
            n * 2_654_435_761
        };
        let (out, changed) = rekey_ids(src, &opts, &mut taken, &mut rng);
        assert_eq!(changed.len(), 1);
        let new = &changed[0].1;
        assert!(out.contains(&format!("- a ^{new}")), "{out}");
        assert!(out.contains(&format!("((({new}))")) || out.contains(&format!("(({new}))")));
        assert!(!out.contains("abc123"));
        assert!(out.contains("^zzz999"), "an id without a clash stays");
        assert!(taken.contains("zzz999") && taken.contains(new));
        assert_eq!(referenced_ids(&out, &opts), [new.clone()].into());
    }
}
