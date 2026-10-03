//! Model-based property test (P1.14): random tree + random op sequence, compared with a simple
#![allow(clippy::needless_late_init, clippy::bind_instead_of_map)]
//! in-memory reference tree; also inverse patches and "no new diagnostics".
use notatnik_core::{Block, BlockOp, BlockRef, apply, apply_patches, invert_patches, parse_page};
use proptest::prelude::*;

#[derive(Debug, Clone, PartialEq)]
struct Node {
    id: usize,
    text: String,
    children: Vec<Node>,
}

fn serialize(nodes: &[Node], depth: usize, out: &mut String) {
    for n in nodes {
        out.push_str(&"  ".repeat(depth));
        out.push_str("- ");
        out.push_str(&n.text);
        out.push('\n');
        serialize(&n.children, depth + 1, out);
    }
}

fn from_page(blocks: &[Block], src: &str) -> Vec<(String, Vec<usize>)> {
    // flattened (text, path) in preorder, enough to compare structure
    fn walk(bs: &[Block], src: &str, path: &mut Vec<usize>, out: &mut Vec<(String, Vec<usize>)>) {
        for (i, b) in bs.iter().enumerate() {
            path.push(i);
            out.push((b.first_line.slice(src).to_string(), path.clone()));
            walk(&b.children, src, path, out);
            path.pop();
        }
    }
    let mut out = Vec::new();
    walk(blocks, src, &mut Vec::new(), &mut out);
    out
}

fn flatten(nodes: &[Node]) -> Vec<(String, Vec<usize>)> {
    fn walk(ns: &[Node], path: &mut Vec<usize>, out: &mut Vec<(String, Vec<usize>)>) {
        for (i, n) in ns.iter().enumerate() {
            path.push(i);
            out.push((n.text.clone(), path.clone()));
            walk(&n.children, path, out);
            path.pop();
        }
    }
    let mut out = Vec::new();
    walk(nodes, &mut Vec::new(), &mut out);
    out
}

fn children_at<'a>(root: &'a mut Vec<Node>, path: &[usize]) -> &'a mut Vec<Node> {
    let mut cur = root;
    for i in path {
        cur = &mut cur[*i].children;
    }
    cur
}

fn node_at<'a>(root: &'a [Node], path: &[usize]) -> &'a Node {
    let mut cur = &root[path[0]];
    for i in &path[1..] {
        cur = &cur.children[*i];
    }
    cur
}

fn find_path(nodes: &[Node], id: usize, path: &mut Vec<usize>) -> bool {
    for (i, n) in nodes.iter().enumerate() {
        path.push(i);
        if n.id == id || find_path(&n.children, id, path) {
            return true;
        }
        path.pop();
    }
    false
}

fn tree_strategy() -> impl Strategy<Value = Vec<Node>> {
    let leaf = "[a-z]{1,6}".prop_map(|t| Node {
        id: 0,
        text: t,
        children: vec![],
    });
    let node = leaf.prop_recursive(3, 12, 3, |inner| {
        ("[a-z]{1,6}", prop::collection::vec(inner, 0..3)).prop_map(|(t, c)| Node {
            id: 0,
            text: t,
            children: c,
        })
    });
    prop::collection::vec(node, 1..5).prop_map(|mut v| {
        let mut next = 0;
        fn number(ns: &mut [Node], next: &mut usize) {
            for n in ns {
                n.id = *next;
                *next += 1;
                number(&mut n.children, next);
            }
        }
        number(&mut v, &mut next);
        v
    })
}

fn all_paths(nodes: &[Node]) -> Vec<Vec<usize>> {
    flatten(nodes).into_iter().map(|(_, p)| p).collect()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]
    #[test]
    fn ops_match_reference(tree in tree_strategy(), steps in prop::collection::vec((0u8..6, any::<usize>(), any::<usize>()), 20)) {
        let mut model = tree;
        let mut next_id = 10_000;
        let mut src = String::new();
        serialize(&model, 0, &mut src);
        for (step, (kind, a, b)) in steps.into_iter().enumerate() {
            let paths = all_paths(&model);
            let page = parse_page(&src);
            let pick = |x: usize| paths.get(x % paths.len().max(1)).cloned();
            let op: Option<BlockOp>;
            match kind {
                0 => {
                    let mut parents: Vec<Vec<usize>> = vec![vec![]];
                    parents.extend(paths.iter().cloned());
                    let parent = parents[a % parents.len()].clone();
                    let len = if parent.is_empty() { model.len() } else { node_at(&model, &parent).children.len() };
                    let index = b % (len + 1);
                    let text = format!("n{step}");
                    children_at(&mut model, &parent).insert(index, Node { id: next_id, text: text.clone(), children: vec![] });
                    next_id += 1;
                    op = Some(BlockOp::Insert { parent, index, text });
                }
                1 => {
                    op = pick(a).map(|p| {
                        let text = format!("u{step}");
                        let (pp, last) = p.split_at(p.len() - 1);
                        children_at(&mut model, pp)[last[0]].text = text.clone();
                        BlockOp::UpdateText { block: BlockRef::at(&p), text }
                    });
                }
                2 => {
                    op = pick(a).map(|p| {
                        let (pp, last) = p.split_at(p.len() - 1);
                        children_at(&mut model, pp).remove(last[0]);
                        BlockOp::Delete { block: BlockRef::at(&p) }
                    });
                }
                3 => {
                    op = pick(a).filter(|p| p.last().copied().unwrap_or(0) > 0).map(|p| {
                        let (pp, last) = p.split_at(p.len() - 1);
                        let sibs = children_at(&mut model, pp);
                        let n = sibs.remove(last[0]);
                        sibs[last[0] - 1].children.push(n);
                        BlockOp::Indent { block: BlockRef::at(&p) }
                    });
                }
                4 => {
                    op = pick(a).filter(|p| p.len() >= 2).map(|p| {
                        let (pp, last) = p.split_at(p.len() - 1);
                        let i = last[0];
                        let parent_children = children_at(&mut model, pp);
                        let mut n = parent_children.remove(i);
                        let later = parent_children.split_off(i);
                        n.children.extend(later);
                        let (ppp, pl) = pp.split_at(pp.len() - 1);
                        children_at(&mut model, ppp).insert(pl[0] + 1, n);
                        BlockOp::Outdent { block: BlockRef::at(&p) }
                    });
                }
                _ => {
                    op = pick(a).and_then(|p| {
                        let mut parents: Vec<Vec<usize>> = vec![vec![]];
                        parents.extend(paths.iter().filter(|q| !q.starts_with(&p)).cloned());
                        let np = parents[b % parents.len()].clone();
                        let moved_id = node_at(&model, &p).id;
                        let parent_id = if np.is_empty() { None } else { Some(node_at(&model, &np).id) };
                        let (pp, last) = p.split_at(p.len() - 1);
                        let same_parent = pp == np.as_slice();
                        let len = if np.is_empty() { model.len() } else { node_at(&model, &np).children.len() };
                        let len_after = len - usize::from(same_parent);
                        let index = (a / 7).wrapping_add(b) % (len_after + 1);
                        let n = children_at(&mut model, pp).remove(last[0]);
                        let mut np_after = Vec::new();
                        if let Some(id) = parent_id {
                            assert!(find_path(&model, id, &mut np_after));
                        }
                        children_at(&mut model, &np_after).insert(index, n);
                        let _ = moved_id;
                        Some(BlockOp::Move { block: BlockRef::at(&p), new_parent: np, index })
                    });
                }
            }
            let Some(op) = op else { continue };
            let result = apply(&src, &page, &op).map_err(|e| TestCaseError::fail(format!("{op:?}: {e}\n{src}")))?;
            // inverse
            prop_assert_eq!(apply_patches(&result.new_source, &invert_patches(&result.patches)), src.clone());
            let reparsed = parse_page(&result.new_source);
            prop_assert!(reparsed.diagnostics.is_empty());
            prop_assert_eq!(from_page(&reparsed.blocks, &result.new_source), flatten(&model), "after {:?}\nsource:\n{}\nnew:\n{}", op, src, result.new_source);
            src = result.new_source;
        }
    }
}
