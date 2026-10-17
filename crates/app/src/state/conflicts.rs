//! Conflict copies and their resolution (ST-6): a three-way merge of the page and its copy, and the writing of
//! the result. The merge itself is `notatnik_core::merge`; the common ancestor comes from the sync (the page as
//! it was at the last sync) or, failing that, is guessed from the page's version history.

use notatnik_core::{Choice, ConflictKind, MergeResult, merge_with};

use super::*;

/// The page a conflict copy belongs to: `dir/name (conflicted copy ...).md` is a copy of `dir/name.md`.
fn original_of(copy: &str) -> Option<String> {
    copy.find(" (conflicted copy ")
        .map(|i| format!("{}.md", &copy[..i]))
}

fn choices(list: &[ConflictChoice]) -> Vec<Choice> {
    list.iter()
        .map(|c| match c.pick.as_str() {
            "theirs" => Choice::Theirs,
            "both" => Choice::Both,
            "text" => Choice::Text(c.text.clone().unwrap_or_default()),
            _ => Choice::Ours,
        })
        .collect()
}

/// A conflict copy's page (path, file), the copy's text, and the common ancestor with where it came from.
type ConflictParts = (
    Option<String>,
    Option<notatnik_graph::PageFile>,
    String,
    Option<(String, &'static str)>,
);

impl AppState {
    /// The page, the copy and the common ancestor of a conflict copy; `Err(None)`-like cases are given as
    /// `Ok(Err(view))` with what could be found, when there is no ancestor.
    fn conflict_inputs(o: &Open, path: &str) -> ApiResult<ConflictParts> {
        if !o.graph.conflict_files()?.iter().any(|c| c == path) {
            return Err(ApiError::new("not_found", "not a conflict file"));
        }
        let copy = std::fs::read_to_string(o.graph.root().join(path))
            .map_err(|e| ApiError::new("internal", e.to_string()))?;
        let orig = original_of(path);
        let page = orig.as_deref().and_then(|r| o.graph.read_page(r).ok());
        let mut base = None;
        if page.is_some() {
            let bases = notatnik_sync::Bases::new(Some(syncing::bases_dir(o.graph.root())));
            base = bases
                .ancestor(path)
                .and_then(|b| String::from_utf8(b).ok())
                .map(|b| (b, "synced"));
        }
        if let (None, Some(p), Some(r)) = (&base, &page, &orig) {
            base = guess_base(o, r, &p.text, &copy);
        }
        Ok((orig, page, copy, base))
    }

    fn merge_for(
        o: &Open,
        path: &str,
        picks: &[ConflictChoice],
    ) -> ApiResult<(ConflictMergeView, Option<String>)> {
        let (orig, page, copy, base) = Self::conflict_inputs(o, path)?;
        let mut view = ConflictMergeView {
            path: path.to_string(),
            original: orig
                .as_deref()
                .and_then(notatnik_core::path_to_page)
                .map(|n| n.to_string()),
            original_path: orig.clone(),
            original_text: page.as_ref().map(|p| p.text.clone()),
            original_hash: page.as_ref().map(|p| p.hash.clone()),
            copy_text: copy.clone(),
            base: "none".into(),
            merged_text: None,
            clean: false,
            conflicts: Vec::new(),
        };
        let (Some(orig), Some(page), Some((base, source))) = (orig, page, base) else {
            return Ok((view, None));
        };
        let merged: MergeResult = merge_with(&base, &page.text, &copy, &choices(picks))
            .map_err(|_| ApiError::new("invalid", "the page cannot be merged automatically"))?;
        view.base = source.into();
        view.clean = merged.is_clean();
        view.merged_text = Some(merged.merged_text);
        view.conflicts = merged
            .conflicts
            .into_iter()
            .map(|c| ConflictItem {
                index: c.index as u32,
                kind: match c.kind {
                    ConflictKind::Edit => "edit",
                    ConflictKind::EditDelete => "edit_delete",
                    ConflictKind::Text => "text",
                }
                .into(),
                label: c.label,
                base: c.base,
                ours: c.ours,
                theirs: c.theirs,
            })
            .collect();
        Ok((view, Some(orig)))
    }

    /// `conflict_merge`: the conflict copy merged with its page.
    pub fn conflict_merge(&self, a: ConflictMergeArgs) -> ApiResult<ConflictMergeView> {
        self.with_open(|o| Ok(Self::merge_for(o, &a.path, &a.choices)?.0))
    }

    /// `resolve_conflict`: writes the merge (or keeps the page, or takes the copy) and moves the copy to the
    /// trash. Returns the conflict copies that are left.
    pub fn resolve_conflict(&self, a: ResolveConflictArgs) -> ApiResult<StringList> {
        self.with_open(|o| {
            let (view, inputs) = Self::merge_for(o, &a.path, &a.choices)?;
            let guard = a.expected_hash.as_deref().or(view.original_hash.as_deref());
            match a.mode.as_str() {
                "keep_original" => {}
                "use_copy" => {
                    let orig = view.original_path.as_deref().ok_or_else(|| {
                        ApiError::new("invalid", "the page of this copy is unknown")
                    })?;
                    o.graph.write_page(orig, &view.copy_text, guard)?;
                }
                "merge" => {
                    let (Some(orig), Some(text)) = (&inputs, &view.merged_text) else {
                        return Err(ApiError::new(
                            "invalid",
                            "there is no common ancestor to merge with",
                        ));
                    };
                    o.graph.write_page(orig, text, guard)?;
                }
                _ => return Err(ApiError::new("invalid", "unknown way to resolve")),
            }
            o.graph.trash_conflict_file(&a.path)?;
            notatnik_sync::Bases::new(Some(syncing::bases_dir(o.graph.root())))
                .remove_ancestor(&a.path);
            Ok(StringList {
                items: o.graph.conflict_files()?,
            })
        })
    }
}

/// With no ancestor from the sync: the recorded version of the page that merges with the fewest conflicts. A
/// version equal to either side is no ancestor and is skipped.
fn guess_base(o: &Open, orig: &str, ours: &str, copy: &str) -> Option<(String, &'static str)> {
    let mut best: Option<(usize, String)> = None;
    for v in o.graph.page_versions(orig).ok()?.into_iter().take(40) {
        let Ok(text) = o.graph.read_page_version(orig, &v.id) else {
            continue;
        };
        if text == ours || text == copy {
            continue;
        }
        let Ok(m) = notatnik_core::merge(&text, ours, copy) else {
            continue;
        };
        if best.as_ref().is_none_or(|(n, _)| m.conflicts.len() < *n) {
            best = Some((m.conflicts.len(), text));
        }
    }
    best.map(|(_, t)| (t, "history"))
}
