//! Three-way merge of a page (ST-6, P5.1).
//!
//! `merge(base, ours, theirs)` works on blocks, not lines. The block trees of both sides are aligned with the
//! base (same `^id`, then identical text, then a similar first line, then the only unmatched block left in a
//! gap); a block changed on one side takes that side's version, a block changed on both sides is merged by
//! part (first line, properties per key, continuation lines by line), and what cannot be merged is reported as a
//! [`BlockConflict`] that the caller resolves with a [`Choice`].
//!
//! Known limits: a block moved to another parent counts as deleted there and added at the new place; a block
//! that is only reordered is kept in the order of the side that reordered (ours when both did).
//! Nothing here reads files or the clock.

use std::collections::HashMap;

use crate::{Block, parse_page};

/// What kind of difference a [`BlockConflict`] is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictKind {
    /// Both sides changed the same block and the changes could not be combined.
    Edit,
    /// One side deleted a block (or a block's whole subtree) that the other side changed.
    EditDelete,
    /// The text before the first block (front matter), the text after the last block, or the text before the
    /// first bullet differs.
    Text,
}

/// A difference that needs a decision. The texts are the block's own lines (bullet line, properties and
/// continuation lines, without its children), exactly as they are in the files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockConflict {
    /// Position in the list of conflicts of one merge; [`Choice`]s are given in this order.
    pub index: usize,
    /// What differs.
    pub kind: ConflictKind,
    /// The first line of the block, for a title.
    pub label: String,
    /// The common ancestor's text (`None`: the block did not exist there).
    pub base: Option<String>,
    /// Our text (`None`: we deleted the block).
    pub ours: Option<String>,
    /// Their text (`None`: they deleted the block).
    pub theirs: Option<String>,
}

/// How one conflict is resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Choice {
    /// Our version (a deletion, if we deleted the block).
    Ours,
    /// Their version (a deletion, if they deleted the block).
    Theirs,
    /// Both texts, ours first (the block that still exists, when one side deleted it).
    Both,
    /// This text instead of both (the block's own lines).
    Text(String),
}

/// The result of a merge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeResult {
    /// The merged file. Each conflict is resolved with its `Choice` (ours when none is given; a block that was
    /// changed on one side and deleted on the other is kept).
    pub merged_text: String,
    /// What needs a decision; empty for a clean merge.
    pub conflicts: Vec<BlockConflict>,
}

impl MergeResult {
    /// True when nothing needs a decision.
    pub fn is_clean(&self) -> bool {
        self.conflicts.is_empty()
    }
}

/// Why a merge is not attempted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MergeError {
    /// A side cannot be taken apart and put together again without changing a byte (the caller falls back to
    /// keeping both files).
    Unmergeable,
}

/// Merges `ours` and `theirs`, both derived from `base`, and resolves every conflict with ours.
pub fn merge(base: &str, ours: &str, theirs: &str) -> Result<MergeResult, MergeError> {
    merge_with(base, ours, theirs, &[])
}

/// Like [`merge`], with the decision for conflict `i` in `choices[i]`.
pub fn merge_with(
    base: &str,
    ours: &str,
    theirs: &str,
    choices: &[Choice],
) -> Result<MergeResult, MergeError> {
    let (b, o, t) = (Doc::read(base)?, Doc::read(ours)?, Doc::read(theirs)?);
    let mut cx = Cx {
        conflicts: Vec::new(),
        choices,
    };
    let header = cx.text(&b.header, &o.header, &t.header);
    let pre = cx.preamble(b.pre.as_ref(), o.pre.as_ref(), t.pre.as_ref());
    let body = merge_children(&b.nodes, &o.nodes, &t.nodes, &mut cx);
    let tail = cx.text(&b.tail, &o.tail, &t.tail);

    let mut out = String::new();
    push(&mut out, &header);
    render(&mut out, &pre);
    render(&mut out, &body);
    push(&mut out, &tail);
    Ok(MergeResult {
        merged_text: out,
        conflicts: cx.conflicts,
    })
}

// ---- the parts of a page ----

/// One block of a page with the text around it, so that putting the parts together gives the file back.
#[derive(Debug, Clone)]
struct N {
    /// Text between the end of the previous block and the start of this one (blank lines).
    lead: String,
    head: String,
    props: String,
    rest: String,
    /// `head + props + rest`, the block's own lines.
    key: String,
    id: Option<String>,
    children: Vec<N>,
}

struct Doc {
    header: String,
    pre: Option<N>,
    nodes: Vec<N>,
    tail: String,
}

impl Doc {
    fn read(src: &str) -> Result<Doc, MergeError> {
        let page = parse_page(src);
        let first = page
            .all_blocks()
            .first()
            .map_or(src.len(), |b| b.own_span.start);
        let mut pos = first;
        let pre = page.preamble.as_ref().map(|b| node(b, src, &mut pos));
        let nodes: Vec<N> = page.blocks.iter().map(|b| node(b, src, &mut pos)).collect();
        let doc = Doc {
            header: src.get(..first).unwrap_or("").to_string(),
            pre,
            nodes,
            tail: src.get(pos..).unwrap_or("").to_string(),
        };
        // the pieces must give the file back byte for byte, or a merge could lose text
        let mut again = String::new();
        push(&mut again, &doc.header);
        if let Some(p) = &doc.pre {
            render(&mut again, &[to_m(p)]);
        }
        render(&mut again, &doc.nodes.iter().map(to_m).collect::<Vec<_>>());
        push(&mut again, &doc.tail);
        if again == src {
            Ok(doc)
        } else {
            Err(MergeError::Unmergeable)
        }
    }
}

fn node(b: &Block, src: &str, pos: &mut usize) -> N {
    let (s, e) = (
        b.own_span.start.min(src.len()),
        b.own_span.end.min(src.len()),
    );
    let lead = if s >= *pos {
        src.get(*pos..s).unwrap_or("")
    } else {
        ""
    };
    let s = s.max(*pos);
    let e = e.max(s);
    let p0 = b
        .props_span
        .map(|p| p.start)
        .or(b.rest_span.map(|r| r.start))
        .unwrap_or(e)
        .clamp(s, e);
    let p1 = b.props_span.map_or(p0, |p| p.end).clamp(p0, e);
    let part = |a: usize, z: usize| src.get(a..z).unwrap_or("").to_string();
    let n = N {
        lead: lead.to_string(),
        head: part(s, p0),
        props: part(p0, p1),
        rest: part(p1, e),
        key: part(s, e),
        id: b.block_id.as_ref().map(|i| i.id.clone()),
        children: Vec::new(),
    };
    *pos = e;
    let children = b.children.iter().map(|c| node(c, src, pos)).collect();
    N { children, ..n }
}

/// A block of the result.
#[derive(Debug, Clone)]
struct M {
    lead: String,
    own: String,
    kids: Vec<M>,
}

fn to_m(n: &N) -> M {
    M {
        lead: n.lead.clone(),
        own: n.key.clone(),
        kids: n.children.iter().map(to_m).collect(),
    }
}

/// Appends `s`, starting a new line first if the text so far ends in the middle of one.
fn push(out: &mut String, s: &str) {
    if !s.is_empty() && !out.is_empty() && !out.ends_with('\n') && !s.starts_with('\n') {
        out.push('\n');
    }
    out.push_str(s);
}

fn render(out: &mut String, ms: &[M]) {
    for m in ms {
        push(out, &m.lead);
        push(out, &m.own);
        render(out, &m.kids);
    }
}

/// Whole blocks are the same: own text and, recursively, children.
fn same(a: &N, b: &N) -> bool {
    a.key == b.key
        && a.children.len() == b.children.len()
        && a.children.iter().zip(&b.children).all(|(x, y)| same(x, y))
}

// ---- conflicts and their resolution ----

struct Cx<'a> {
    conflicts: Vec<BlockConflict>,
    choices: &'a [Choice],
}

impl Cx<'_> {
    /// Records a conflict and returns the choice for it.
    fn conflict(
        &mut self,
        kind: ConflictKind,
        base: Option<&str>,
        ours: Option<&str>,
        theirs: Option<&str>,
    ) -> Option<Choice> {
        let index = self.conflicts.len();
        let label = ours
            .or(theirs)
            .or(base)
            .and_then(|t| t.lines().next())
            .unwrap_or("")
            .trim()
            .to_string();
        self.conflicts.push(BlockConflict {
            index,
            kind,
            label,
            base: base.map(str::to_string),
            ours: ours.map(str::to_string),
            theirs: theirs.map(str::to_string),
        });
        self.choices.get(index).cloned()
    }

    /// Text outside the blocks: a side that changed wins, a difference is a conflict.
    fn text(&mut self, b: &str, o: &str, t: &str) -> String {
        if let Some(s) = three(b, o, t) {
            return s.to_string();
        }
        if let Some(s) = diff3(b, o, t) {
            return s;
        }
        match self.conflict(ConflictKind::Text, Some(b), Some(o), Some(t)) {
            Some(Choice::Theirs) => t.to_string(),
            Some(Choice::Text(s)) => s,
            _ => o.to_string(),
        }
    }

    fn preamble(&mut self, b: Option<&N>, o: Option<&N>, t: Option<&N>) -> Vec<M> {
        fn key(n: Option<&N>) -> Option<&str> {
            n.map(|n| n.key.as_str())
        }
        let one = |own: String| M {
            lead: String::new(),
            own,
            kids: Vec::new(),
        };
        let own = |n: Option<&N>| n.map(|n| one(n.key.clone()));
        if key(o) == key(t) || key(t) == key(b) {
            return own(o).into_iter().collect();
        }
        if key(o) == key(b) {
            return own(t).into_iter().collect();
        }
        if let (Some(b), Some(o), Some(t)) = (b, o, t)
            && let Some(text) = merge_own(b, o, t)
        {
            return vec![one(text)];
        }
        match self.conflict(ConflictKind::Edit, key(b), key(o), key(t)) {
            Some(Choice::Theirs) => own(t).into_iter().collect(),
            Some(Choice::Both) => own(o).into_iter().chain(own(t)).collect(),
            Some(Choice::Text(s)) => vec![one(s)],
            _ => own(o).into_iter().collect(),
        }
    }
}

/// The text that one side changed, or that both left alone or changed the same way.
fn three<'a, T: PartialEq + ?Sized>(b: &T, o: &'a T, t: &'a T) -> Option<&'a T> {
    if o == t || t == b {
        Some(o)
    } else if o == b {
        Some(t)
    } else {
        None
    }
}

// ---- merging blocks ----

/// A block both sides still have: its own lines by part, then its children.
fn merge_node(b: &N, o: &N, t: &N, cx: &mut Cx) -> Vec<M> {
    let lead = three(b.lead.as_str(), o.lead.as_str(), t.lead.as_str())
        .unwrap_or(o.lead.as_str())
        .to_string();
    let own = merge_own(b, o, t);
    let choice = if own.is_none() {
        cx.conflict(ConflictKind::Edit, Some(&b.key), Some(&o.key), Some(&t.key))
    } else {
        None
    };
    let kids = merge_children(&b.children, &o.children, &t.children, cx);
    let make = |own: String, kids: Vec<M>| M {
        lead: lead.clone(),
        own,
        kids,
    };
    match own {
        Some(text) => vec![make(text, kids)],
        None => match choice {
            Some(Choice::Theirs) => vec![make(t.key.clone(), kids)],
            Some(Choice::Both) => vec![
                make(o.key.clone(), kids),
                M {
                    lead: String::new(),
                    own: t.key.clone(),
                    kids: Vec::new(),
                },
            ],
            Some(Choice::Text(s)) => vec![make(s, kids)],
            _ => vec![make(o.key.clone(), kids)],
        },
    }
}

/// The own lines of a block changed on both sides, combined part by part; `None` if a part conflicts.
fn merge_own(b: &N, o: &N, t: &N) -> Option<String> {
    if let Some(k) = three(b.key.as_str(), o.key.as_str(), t.key.as_str()) {
        return Some(k.to_string());
    }
    let head = three(b.head.as_str(), o.head.as_str(), t.head.as_str())?;
    let props = merge_props(&b.props, &o.props, &t.props)?;
    let rest = match three(b.rest.as_str(), o.rest.as_str(), t.rest.as_str()) {
        Some(r) => r.to_string(),
        None => diff3(&b.rest, &o.rest, &t.rest)?,
    };
    let mut out = String::new();
    push(&mut out, head);
    push(&mut out, &props);
    push(&mut out, &rest);
    Some(out)
}

/// `key:: value` lines by key: a key changed on one side takes that change, added keys are all kept (ours
/// first), and a key changed differently on both sides conflicts.
fn merge_props(b: &str, o: &str, t: &str) -> Option<String> {
    if let Some(p) = three(b, o, t) {
        return Some(p.to_string());
    }
    let (kb, ko, kt) = (keyed(b), keyed(o), keyed(t));
    let (Some(kb), Some(ko), Some(kt)) = (kb, ko, kt) else {
        return diff3(b, o, t);
    };
    fn get<'a>(v: &'a [(String, String)], k: &str) -> Option<&'a str> {
        v.iter().find(|(x, _)| x == k).map(|(_, l)| l.as_str())
    }
    let mut keys: Vec<&str> = ko.iter().map(|(k, _)| k.as_str()).collect();
    for (k, _) in &kt {
        if !keys.contains(&k.as_str()) {
            keys.push(k);
        }
    }
    let mut out = String::new();
    for k in keys {
        let (vb, vo, vt) = (get(&kb, k), get(&ko, k), get(&kt, k));
        if let Some(line) = three(&vb, &vo, &vt).copied().flatten() {
            push(&mut out, line);
        } else if three(&vb, &vo, &vt).is_none() {
            return None;
        }
    }
    Some(out)
}

/// The lines of a properties part by key; `None` when a line has no key or a key is repeated.
fn keyed(text: &str) -> Option<Vec<(String, String)>> {
    let mut out: Vec<(String, String)> = Vec::new();
    for line in text.split_inclusive('\n') {
        let (key, _) = line.trim_start().split_once("::")?;
        if key.is_empty() || key.contains(char::is_whitespace) || out.iter().any(|(k, _)| k == key)
        {
            return None;
        }
        out.push((key.to_string(), line.to_string()));
    }
    Some(out)
}

/// Aligns two sibling lists with the base's: for each base block the index of its partner on the other side.
fn align(base: &[N], other: &[N]) -> Vec<Option<usize>> {
    let mut m: Vec<Option<usize>> = vec![None; base.len()];
    let mut used = vec![false; other.len()];
    // 1. the same block ID (unique on both sides)
    fn unique(list: &[N]) -> HashMap<&str, Option<usize>> {
        let mut seen: HashMap<&str, Option<usize>> = HashMap::new();
        for (i, n) in list.iter().enumerate() {
            if let Some(id) = n.id.as_deref() {
                seen.entry(id).and_modify(|v| *v = None).or_insert(Some(i));
            }
        }
        seen
    }
    let (ib, io) = (unique(base), unique(other));
    for (i, n) in base.iter().enumerate() {
        let Some(id) = n.id.as_deref() else { continue };
        if let (Some(Some(bi)), Some(Some(oi))) = (ib.get(id), io.get(id))
            && *bi == i
            && !used[*oi]
        {
            m[i] = Some(*oi);
            used[*oi] = true;
        }
    }
    // 2. identical text, in order
    let ub: Vec<usize> = (0..base.len()).filter(|i| m[*i].is_none()).collect();
    let uo: Vec<usize> = (0..other.len()).filter(|j| !used[*j]).collect();
    let ka: Vec<&str> = ub.iter().map(|i| base[*i].key.as_str()).collect();
    let kb: Vec<&str> = uo.iter().map(|j| other[*j].key.as_str()).collect();
    let pairs = lcs(&ka, &kb);
    for (x, y) in &pairs {
        m[ub[*x]] = Some(uo[*y]);
        used[uo[*y]] = true;
    }
    // 3. in the gaps between those: a similar first line, or the one block left on both sides
    let mut ends = pairs.clone();
    ends.push((ub.len(), uo.len()));
    let (mut px, mut py) = (0, 0);
    for (x, y) in ends {
        let (gb, go) = (&ub[px..x], &uo[py..y]);
        let mut from = 0;
        for bi in gb {
            let hit = (from..go.len()).find(|k| {
                (gb.len() == 1 && go.len() == 1) || similar(&base[*bi].head, &other[go[*k]].head)
            });
            if let Some(k) = hit {
                m[*bi] = Some(go[k]);
                used[go[k]] = true;
                from = k + 1;
            }
        }
        px = x + 1;
        py = y + 1;
    }
    m
}

/// Two first lines share at least half of their words.
fn similar(a: &str, b: &str) -> bool {
    let words = |s: &str| -> Vec<String> {
        let mut v: Vec<String> = s
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| !w.is_empty())
            .map(str::to_lowercase)
            .collect();
        v.sort();
        v.dedup();
        v
    };
    let (wa, wb) = (words(a), words(b));
    let common = wa.iter().filter(|w| wb.contains(w)).count();
    let all = wa.len() + wb.len() - common;
    all > 0 && common * 2 >= all
}

/// The siblings of a block that both sides kept: merged, with additions and deletions.
fn merge_children(base: &[N], ours: &[N], theirs: &[N], cx: &mut Cx) -> Vec<M> {
    if ours.is_empty() && theirs.is_empty() && base.is_empty() {
        return Vec::new();
    }
    let (ao, at) = (align(base, ours), align(base, theirs));
    // per side: the base index of each of its blocks, and the blocks that are new with their anchor (the base
    // block before them on that side)
    let side = |a: &[Option<usize>], len: usize| {
        let mut rev: Vec<Option<usize>> = vec![None; len];
        for (b, o) in a.iter().enumerate() {
            if let Some(o) = o {
                rev[*o] = Some(b);
            }
        }
        let mut adds: Vec<(Option<usize>, usize)> = Vec::new();
        let mut last = None;
        for (i, r) in rev.iter().enumerate() {
            match r {
                Some(b) => last = Some(*b),
                None => adds.push((last, i)),
            }
        }
        let order: Vec<usize> = rev.iter().flatten().copied().collect();
        (adds, order)
    };
    let (adds_o, ord_o) = side(&ao, ours.len());
    let (adds_t, ord_t) = side(&at, theirs.len());
    let increasing = |v: &[usize]| v.windows(2).all(|w| w[0] < w[1]);
    // the order of the side that moved blocks (ours when both did)
    let mut order: Vec<usize> = if !increasing(&ord_o) {
        ord_o
    } else if !increasing(&ord_t) {
        ord_t
    } else {
        (0..base.len()).collect()
    };
    for b in 0..base.len() {
        if !order.contains(&b) {
            let at_ = if b == 0 {
                0
            } else {
                order.iter().position(|x| *x == b - 1).map_or(0, |p| p + 1)
            };
            order.insert(at_, b);
        }
    }

    let adds = |anchor: Option<usize>| -> Vec<M> {
        let o: Vec<&N> = adds_o
            .iter()
            .filter(|(a, _)| *a == anchor)
            .map(|(_, i)| &ours[*i])
            .collect();
        let t = adds_t
            .iter()
            .filter(|(a, _)| *a == anchor)
            .map(|(_, i)| &theirs[*i])
            .filter(|n| !o.iter().any(|x| same(x, n)));
        o.iter().copied().chain(t).map(to_m).collect()
    };

    let mut out = adds(None);
    for b in order {
        match (ao[b], at[b]) {
            (Some(i), Some(j)) => out.extend(merge_node(&base[b], &ours[i], &theirs[j], cx)),
            (None, None) => {}
            (Some(i), None) => {
                if !same(&base[b], &ours[i]) {
                    out.extend(edit_delete(&base[b], Some(&ours[i]), None, cx));
                }
            }
            (None, Some(j)) => {
                if !same(&base[b], &theirs[j]) {
                    out.extend(edit_delete(&base[b], None, Some(&theirs[j]), cx));
                }
            }
        }
        out.extend(adds(Some(b)));
    }
    out
}

/// One side changed a block that the other deleted: kept unless the decision is the deletion.
fn edit_delete(base: &N, ours: Option<&N>, theirs: Option<&N>, cx: &mut Cx) -> Vec<M> {
    let Some(kept) = ours.or(theirs) else {
        return Vec::new();
    };
    let choice = cx.conflict(
        ConflictKind::EditDelete,
        Some(&base.key),
        ours.map(|n| n.key.as_str()),
        theirs.map(|n| n.key.as_str()),
    );
    let mut m = to_m(kept);
    match choice {
        Some(Choice::Ours) if ours.is_none() => Vec::new(),
        Some(Choice::Theirs) if theirs.is_none() => Vec::new(),
        Some(Choice::Text(s)) => {
            m.own = s;
            vec![m]
        }
        _ => vec![m],
    }
}

// ---- sequences ----

/// The longest common subsequence of two lists, as index pairs. Equal heads and tails are paired first, so a
/// small change in a long list is cheap; two long lists that differ everywhere in the middle are not compared.
fn lcs<T: PartialEq>(a: &[T], b: &[T]) -> Vec<(usize, usize)> {
    let mut pre = 0;
    while pre < a.len() && pre < b.len() && a[pre] == b[pre] {
        pre += 1;
    }
    let mut post = 0;
    while post < a.len() - pre
        && post < b.len() - pre
        && a[a.len() - 1 - post] == b[b.len() - 1 - post]
    {
        post += 1;
    }
    let (ma, mb) = (&a[pre..a.len() - post], &b[pre..b.len() - post]);
    let mut out: Vec<(usize, usize)> = (0..pre).map(|i| (i, i)).collect();
    if !ma.is_empty() && !mb.is_empty() && ma.len() * mb.len() <= 4_000_000 {
        let w = mb.len() + 1;
        let mut t = vec![0u32; (ma.len() + 1) * w];
        for i in (0..ma.len()).rev() {
            for j in (0..mb.len()).rev() {
                t[i * w + j] = if ma[i] == mb[j] {
                    t[(i + 1) * w + j + 1] + 1
                } else {
                    t[(i + 1) * w + j].max(t[i * w + j + 1])
                };
            }
        }
        let (mut i, mut j) = (0, 0);
        while i < ma.len() && j < mb.len() {
            if ma[i] == mb[j] {
                out.push((pre + i, pre + j));
                i += 1;
                j += 1;
            } else if t[(i + 1) * w + j] >= t[i * w + j + 1] {
                i += 1;
            } else {
                j += 1;
            }
        }
    }
    out.extend((0..post).map(|k| (a.len() - post + k, b.len() - post + k)));
    out
}

/// A changed range of the base lines and what replaces it.
struct Hunk<'a> {
    from: usize,
    to: usize,
    with: Vec<&'a str>,
}

fn hunks<'a>(base: &[&str], side: &[&'a str]) -> Vec<Hunk<'a>> {
    let mut pairs = lcs(base, side);
    pairs.push((base.len(), side.len()));
    let (mut bi, mut si) = (0, 0);
    let mut out = Vec::new();
    for (pb, ps) in pairs {
        if bi < pb || si < ps {
            out.push(Hunk {
                from: bi,
                to: pb,
                with: side[si..ps].to_vec(),
            });
        }
        bi = pb + 1;
        si = ps + 1;
    }
    out
}

/// Merges lines of text that both sides changed: changes in different places are combined, equal changes
/// are taken once, anything that touches the same lines is `None`.
fn diff3(b: &str, o: &str, t: &str) -> Option<String> {
    fn lines(s: &str) -> Vec<&str> {
        s.split_inclusive('\n').collect()
    }
    let (lb, lo, lt) = (lines(b), lines(o), lines(t));
    if lb.len() * lo.len().max(lt.len()) > 4_000_000 {
        return None;
    }
    let (ho, ht) = (hunks(&lb, &lo), hunks(&lb, &lt));
    let (mut i, mut j, mut pos) = (0, 0, 0);
    let mut out = String::new();
    let take = |h: &Hunk, pos: &mut usize, out: &mut String| {
        lb[*pos..h.from].iter().for_each(|l| out.push_str(l));
        h.with.iter().for_each(|l| out.push_str(l));
        *pos = h.to;
    };
    while i < ho.len() || j < ht.len() {
        match (ho.get(i), ht.get(j)) {
            (Some(x), Some(y)) if x.from == y.from || (x.from < y.to && y.from < x.to) => {
                if x.from == y.from && x.to == y.to && x.with == y.with {
                    take(x, &mut pos, &mut out);
                    i += 1;
                    j += 1;
                } else {
                    return None;
                }
            }
            (Some(x), y) if y.is_none_or(|y| x.to <= y.from) => {
                take(x, &mut pos, &mut out);
                i += 1;
            }
            (_, Some(y)) => {
                take(y, &mut pos, &mut out);
                j += 1;
            }
            (Some(x), None) => {
                take(x, &mut pos, &mut out);
                i += 1;
            }
            (None, None) => break,
        }
    }
    lb[pos..].iter().for_each(|l| out.push_str(l));
    Some(out)
}
