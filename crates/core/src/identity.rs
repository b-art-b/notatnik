//! Stable runtime identity for blocks without a `^id` (P1.26).
//!
//! The UI needs stable keys for blocks across edits and external changes. [`BlockIdentity`] keeps
//! the previous parse of a page and matches old blocks to new ones: first same path and same text,
//! then same text anywhere (moves, inserts, whole-file rewrites), then same path with changed text
//! (an edit). Everything else gets a fresh ID. IDs live only for the session; nothing is stored.

use std::collections::{HashMap, VecDeque};

use serde::Serialize;

use crate::{Block, Page};

/// A block's path and its session-stable runtime ID.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Tracked {
    /// Child-index path (`[usize::MAX]` for the preamble).
    pub path: Vec<usize>,
    /// Runtime ID, unique within this tracker.
    pub id: u64,
}

struct Entry {
    path: Vec<usize>,
    hash: String,
    id: u64,
}

/// Matches blocks between successive parses of the same page.
#[derive(Default)]
pub struct BlockIdentity {
    next: u64,
    prev: Vec<Entry>,
}

fn flatten(page: &Page, source: &str) -> Vec<(Vec<usize>, String)> {
    fn walk(b: &Block, path: &mut Vec<usize>, source: &str, out: &mut Vec<(Vec<usize>, String)>) {
        out.push((path.clone(), b.content_hash(source)));
        for (i, c) in b.children.iter().enumerate() {
            path.push(i);
            walk(c, path, source, out);
            path.pop();
        }
    }
    let mut out = Vec::new();
    if let Some(p) = &page.preamble {
        out.push((vec![usize::MAX], p.content_hash(source)));
    }
    for (i, b) in page.blocks.iter().enumerate() {
        walk(b, &mut vec![i], source, &mut out);
    }
    out
}

impl BlockIdentity {
    /// A tracker with no history.
    pub fn new() -> Self {
        Self::default()
    }

    /// Assigns IDs to the blocks of the freshly parsed `page`, reusing IDs of the previous call
    /// where blocks can be matched. Returns one entry per block in document order.
    pub fn assign(&mut self, page: &Page, source: &str) -> Vec<Tracked> {
        let new = flatten(page, source);
        let mut ids: Vec<Option<u64>> = vec![None; new.len()];
        let mut used = vec![false; self.prev.len()];

        // 1. same path, same text
        let mut by_path_hash: HashMap<(&[usize], &str), usize> = HashMap::new();
        for (j, e) in self.prev.iter().enumerate() {
            by_path_hash
                .entry((e.path.as_slice(), e.hash.as_str()))
                .or_insert(j);
        }
        for (i, (path, hash)) in new.iter().enumerate() {
            if let Some(&j) = by_path_hash.get(&(path.as_slice(), hash.as_str())) {
                if !used[j] {
                    used[j] = true;
                    ids[i] = Some(self.prev[j].id);
                }
            }
        }
        // 2. same text anywhere, in document order
        let mut by_hash: HashMap<&str, VecDeque<usize>> = HashMap::new();
        for (j, e) in self.prev.iter().enumerate() {
            if !used[j] {
                by_hash.entry(e.hash.as_str()).or_default().push_back(j);
            }
        }
        for (i, (_, hash)) in new.iter().enumerate() {
            if ids[i].is_some() {
                continue;
            }
            if let Some(j) = by_hash.get_mut(hash.as_str()).and_then(VecDeque::pop_front) {
                used[j] = true;
                ids[i] = Some(self.prev[j].id);
            }
        }
        // 3. same path, changed text (an edit)
        let mut by_path: HashMap<&[usize], usize> = HashMap::new();
        for (j, e) in self.prev.iter().enumerate() {
            if !used[j] {
                by_path.entry(e.path.as_slice()).or_insert(j);
            }
        }
        for (i, (path, _)) in new.iter().enumerate() {
            if ids[i].is_some() {
                continue;
            }
            if let Some(&j) = by_path.get(path.as_slice()) {
                if !used[j] {
                    used[j] = true;
                    ids[i] = Some(self.prev[j].id);
                }
            }
        }
        // 4. fresh IDs
        let mut next = self.next;
        let result: Vec<Tracked> = new
            .iter()
            .zip(ids)
            .map(|((path, _), id)| {
                let id = id.unwrap_or_else(|| {
                    next += 1;
                    next
                });
                Tracked {
                    path: path.clone(),
                    id,
                }
            })
            .collect();
        self.next = next;
        self.prev = new
            .into_iter()
            .zip(&result)
            .map(|((path, hash), t)| Entry {
                path,
                hash,
                id: t.id,
            })
            .collect();
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse_page;

    fn ids_of(t: &mut BlockIdentity, src: &str) -> Vec<(String, u64)> {
        let page = parse_page(src);
        let tracked = t.assign(&page, src);
        let all = page.all_blocks();
        all.iter()
            .zip(tracked)
            .map(|(b, t)| (b.first_line.slice(src).to_string(), t.id))
            .collect()
    }

    fn id(v: &[(String, u64)], text: &str) -> u64 {
        v.iter()
            .find(|(t, _)| t == text)
            .map(|(_, i)| *i)
            .unwrap_or_else(|| panic!("no {text}"))
    }

    const BASE: &str = "- a\n  - a1\n  - a2\n- b\n- c\n";

    #[test]
    fn edit_keeps_other_ids() {
        let mut t = BlockIdentity::new();
        let v0 = ids_of(&mut t, BASE);
        let v1 = ids_of(&mut t, "- a\n  - a1 changed\n  - a2\n- b\n- c\n");
        assert_eq!(id(&v0, "a"), id(&v1, "a"));
        assert_eq!(id(&v0, "a2"), id(&v1, "a2"));
        assert_eq!(id(&v0, "b"), id(&v1, "b"));
        assert_eq!(
            id(&v0, "a1"),
            id(&v1, "a1 changed"),
            "an edited block keeps its id"
        );
    }

    #[test]
    fn insert_at_top_keeps_all_ids() {
        let mut t = BlockIdentity::new();
        let v0 = ids_of(&mut t, BASE);
        let v1 = ids_of(&mut t, &format!("- new first\n{BASE}"));
        for name in ["a", "a1", "a2", "b", "c"] {
            assert_eq!(id(&v0, name), id(&v1, name), "{name}");
        }
        assert!(v0.iter().all(|(_, i)| *i != id(&v1, "new first")));
    }

    #[test]
    fn move_keeps_id() {
        let mut t = BlockIdentity::new();
        let v0 = ids_of(&mut t, BASE);
        let v1 = ids_of(&mut t, "- b\n- a\n  - a1\n  - a2\n- c\n");
        for name in ["a", "a1", "a2", "b", "c"] {
            assert_eq!(id(&v0, name), id(&v1, name), "{name}");
        }
        let v2 = ids_of(&mut t, "- b\n- a\n  - a2\n- c\n  - a1\n");
        assert_eq!(id(&v1, "a1"), id(&v2, "a1"));
    }

    #[test]
    fn whole_file_rewrite_matches_unchanged_blocks() {
        let mut t = BlockIdentity::new();
        let v0 = ids_of(&mut t, BASE);
        // an external tool rewrote the file: different indentation style, extra blank lines, reordered
        let v1 = ids_of(
            &mut t,
            "- c\n\n- a\n    - a2\n    - a1\n\n- b\n- brand new\n",
        );
        for name in ["a", "a1", "a2", "b", "c"] {
            assert_eq!(id(&v0, name), id(&v1, name), "{name}");
        }
    }

    #[test]
    fn delete_and_fresh_ids_are_never_reused() {
        let mut t = BlockIdentity::new();
        let v0 = ids_of(&mut t, BASE);
        let v1 = ids_of(&mut t, "- a\n");
        let v2 = ids_of(&mut t, BASE);
        assert_eq!(id(&v0, "a"), id(&v1, "a"));
        assert_ne!(
            id(&v0, "b"),
            id(&v2, "b"),
            "a deleted block's id is not resurrected"
        );
        let all: std::collections::HashSet<u64> = v2.iter().map(|(_, i)| *i).collect();
        assert_eq!(all.len(), v2.len(), "ids are unique");
    }
}
