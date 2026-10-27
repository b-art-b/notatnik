//! Renaming a page, or moving a whole namespace, with reference rewriting (P1.34, LN-7, NS-8).

use notatnik_core::{
    Block, PageName, ParseOptions, RefKind, TextPatch, apply_patches, page_to_path, parse_page_with,
};

use crate::{Error, Graph, Result};

/// The rewrite of one file.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FileEdit {
    /// Graph-relative path.
    pub path: String,
    /// Hash of the file content the edit was computed from (guard).
    pub base_hash: String,
    /// The text before the rewrite (kept to revert it).
    pub old_text: String,
    /// The text after the rewrite.
    pub new_text: String,
    /// The individual replacements (for previews).
    pub patches: Vec<TextPatch>,
}

/// Everything a rename would do; produced by [`Graph::plan_rename`] and applied by [`Graph::apply_rename`].
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RenamePlan {
    /// Old page name.
    pub old: String,
    /// New page name.
    pub new: String,
    /// `(from, to)` file moves (relative paths).
    pub file_moves: Vec<(String, String)>,
    /// Files whose references are rewritten.
    pub edits: Vec<FileEdit>,
    /// Things the user should know (e.g. frontmatter tags that are not rewritten).
    pub warnings: Vec<String>,
}

/// What `apply_rename` did.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RenameReport {
    /// Moved files.
    pub moved: Vec<(String, String)>,
    /// Rewritten files.
    pub rewritten: Vec<String>,
}

fn plain_tag_ok(name: &str) -> bool {
    name.chars()
        .all(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | '/'))
        && !name.ends_with(['-', '/'])
}

/// Maps the name of a page in the moved subtree (the page itself and everything below it) to its new name.
struct Mapper {
    old_key: String,
    old_segments: usize,
    new: PageName,
    /// Also map the pages below the moved one (a namespace move); false renames only the page itself.
    subtree: bool,
}

impl Mapper {
    fn new(old: &PageName, new: &PageName, subtree: bool) -> Self {
        Mapper {
            old_key: old.lookup_key(),
            old_segments: old.as_str().split('/').count(),
            new: new.clone(),
            subtree,
        }
    }

    /// The new name for `target`, or `None` if it is outside the subtree. The segments below the moved page keep
    /// the spelling the reference was written with.
    fn map(&self, target: &str) -> Option<PageName> {
        let t = PageName::parse(target).ok()?;
        let key = t.lookup_key();
        if key == self.old_key {
            return Some(self.new.clone());
        }
        if self.subtree && key.starts_with(&format!("{}/", self.old_key)) {
            let rest: Vec<&str> = t.as_str().split('/').skip(self.old_segments).collect();
            return PageName::parse(&format!("{}/{}", self.new, rest.join("/"))).ok();
        }
        None
    }
}

fn collect(b: &Block, source: &str, map: &Mapper, out: &mut Vec<TextPatch>) {
    for r in &b.refs {
        if !matches!(r.kind, RefKind::Page | RefKind::Tag | RefKind::PageEmbed) {
            continue;
        }
        let Some(new) = map.map(&r.target) else {
            continue;
        };
        let (range, text) = if r.kind == RefKind::Tag
            && !r.span.slice(source).starts_with("#[[")
            && !plain_tag_ok(new.as_str())
        {
            (r.span, format!("#[[{new}]]"))
        } else {
            (r.target_span, new.as_str().to_string())
        };
        out.push(TextPatch {
            range,
            old_text: range.slice(source).to_string(),
            new_text: text,
        });
    }
    for c in &b.children {
        collect(c, source, map, out);
    }
}

impl Graph {
    /// Computes a rename without touching anything (preview, LN-7). A page with subpages moves with them (NS-8).
    pub fn plan_rename(&self, old: &str, new: &str) -> Result<RenamePlan> {
        self.plan_rename_with(old, new, true)
    }

    /// Like [`Graph::plan_rename`]. With `subpages = false` only the page itself is renamed or moved and the pages
    /// below it stay where they are; that is also the only way to move a page to a name below itself
    /// (`a` → `a/b/c`).
    pub fn plan_rename_with(&self, old: &str, new: &str, subpages: bool) -> Result<RenamePlan> {
        let old_name = PageName::parse(old)
            .map_err(|e| Error::Rename(format!("invalid page name `{old}`: {e}")))?;
        let new_name = PageName::parse(new)
            .map_err(|e| Error::Rename(format!("invalid page name `{new}`: {e}")))?;
        if old_name == new_name {
            return Err(Error::Rename(
                "the new name is the same as the old one".into(),
            ));
        }
        if old_name.journal_date().is_some() || new_name.journal_date().is_some() {
            return Err(Error::Rename("journal pages can't be renamed".into()));
        }
        let old_key = old_name.lookup_key();
        let new_key = new_name.lookup_key();
        if subpages && new_key.starts_with(&format!("{old_key}/")) {
            return Err(Error::Rename(format!(
                "can't move `{old}` with its subpages into itself (`{new}` is below it); \
                 move only this page instead (leave the subpages where they are)"
            )));
        }
        let (info, subtree, referencing) = self.with_index(|i| {
            Ok::<_, Error>((
                i.page_info(old)?,
                if subpages {
                    i.subtree_pages(old)?
                } else {
                    Vec::new()
                },
                if subpages {
                    i.files_referencing_subtree(old)?
                } else {
                    i.files_referencing(old)?
                },
            ))
        })?;
        let info = info.ok_or_else(|| Error::NotFound(old.to_string()))?;
        if PageName::parse(&info.name)
            .map(|n| n.lookup_key())
            .as_deref()
            != Ok(old_key.as_str())
        {
            return Err(Error::Rename(format!(
                "`{old}` is an alias of `{}`; rename that page instead",
                info.name
            )));
        }
        if let Some(existing) = self.with_index(|i| i.page_info(new))?
            && !existing.is_virtual
        {
            return Err(Error::Rename(format!(
                "a page named `{}` already exists",
                existing.name
            )));
        }
        let map = Mapper::new(&old_name, &new_name, subpages);
        let mut file_moves = Vec::new();
        let mut moved_pages = vec![(new_name.clone(), info.file_path.clone())];
        for d in &subtree {
            let dest = map
                .map(&d.name)
                .ok_or_else(|| Error::Rename(format!("`{}` can't be moved to `{new}`", d.name)))?;
            // a page of that name that is not part of the moved subtree blocks the move
            if let Some(existing) = self.with_index(|i| i.page_info(dest.as_str()))?
                && !existing.is_virtual
            {
                return Err(Error::Rename(format!(
                    "a page named `{}` already exists",
                    existing.name
                )));
            }
            moved_pages.push((dest, d.file_path.clone()));
        }
        for (dest, from) in &moved_pages {
            let Some(from) = from else { continue };
            let to = page_to_path(dest);
            if self.root().join(&to).exists() && *from != to {
                return Err(Error::Rename(format!("the file {to} already exists")));
            }
            if *from != to {
                file_moves.push((from.clone(), to));
            }
        }
        let opts: ParseOptions = self.config().parse_options();
        let mut edits = Vec::new();
        let mut warnings = Vec::new();
        for path in referencing {
            let file = self.read_page(&path)?;
            let page = parse_page_with(&file.text, &opts);
            let mut patches = Vec::new();
            if let Some(p) = &page.preamble {
                collect(p, &file.text, &map, &mut patches);
            }
            for b in &page.blocks {
                collect(b, &file.text, &map, &mut patches);
            }
            if let Some(fm) = page.frontmatter.as_ref().filter(|f| f.valid)
                && fm.tags.iter().any(|t| map.map(t).is_some())
            {
                warnings.push(format!(
                    "{path}: frontmatter tags still mention `{old}`; edit them by hand"
                ));
            }
            patches.sort_by_key(|p| p.range.start);
            patches.dedup_by_key(|p: &mut TextPatch| p.range);
            if patches.is_empty() {
                continue;
            }
            let new_text = apply_patches(&file.text, &patches);
            edits.push(FileEdit {
                path,
                base_hash: file.hash,
                old_text: file.text.clone(),
                new_text,
                patches,
            });
        }
        // a page that moves to another folder keeps its relative links and images (`../assets/x.png`) working
        for (from, to) in &file_moves {
            let existing = edits.iter().position(|e: &FileEdit| &e.path == from);
            let (old_text, base_hash, mut patches) = match existing {
                Some(i) => {
                    let e = &edits[i];
                    (e.old_text.clone(), e.base_hash.clone(), e.patches.clone())
                }
                None => match self.read_page(from) {
                    Ok(f) => (f.text, f.hash, Vec::new()),
                    Err(_) => continue,
                },
            };
            let relinks = relink_patches(&old_text, from, to);
            if relinks.is_empty() {
                continue;
            }
            patches.extend(relinks);
            patches.sort_by_key(|p| p.range.start);
            let new_text = apply_patches(&old_text, &patches);
            let edit = FileEdit {
                path: from.clone(),
                base_hash,
                old_text,
                new_text,
                patches,
            };
            match existing {
                Some(i) => edits[i] = edit,
                None => edits.push(edit),
            }
        }
        Ok(RenamePlan {
            old: old_name.to_string(),
            new: new_name.to_string(),
            file_moves,
            edits,
            warnings,
        })
    }

    /// Undoes (as far as it got) a rename that stopped half way: rewritten files get their old text back
    /// (only if they still hold exactly what the rename wrote, so nobody's newer edit is lost) and moved files
    /// go back to their old place. Safe to call on a plan that was never applied or only partly.
    pub fn revert_rename(&self, plan: &RenamePlan) -> Result<Vec<String>> {
        let mut restored = Vec::new();
        for (from, to) in plan.file_moves.iter().rev() {
            let (abs_from, abs_to) = (self.root().join(from), self.root().join(to));
            if abs_to.exists() && !abs_from.exists() {
                if let Some(parent) = abs_from.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::rename(&abs_to, &abs_from)?;
                if let Some(parent) = abs_from.parent() {
                    crate::sync_dir(parent);
                }
                let mut g = self.lock();
                let root = g.root.clone();
                g.index.reindex_file(&root, to)?;
                g.index.reindex_file(&root, from)?;
                g.move_versions(to, from);
                restored.push(from.clone());
            }
        }
        for e in plan.edits.iter().rev() {
            let Ok(now) = self.read_page(&e.path) else {
                continue;
            };
            if now.text == e.new_text {
                self.write_page(&e.path, &e.old_text, Some(&now.hash))?;
                restored.push(e.path.clone());
            }
        }
        Ok(restored)
    }

    /// Applies a plan: rewrites references first (each file with its base-hash guard), moves files last.
    /// If a file conflicts, stops and reports what was already done; nothing is moved in that case.
    pub fn apply_rename(&self, plan: &RenamePlan) -> Result<RenameReport> {
        let mut report = RenameReport::default();
        // the record stays if the app dies half way; it is removed when this returns, whatever the result
        let _journal =
            self.journal_begin(&crate::journal::Pending::Rename { plan: plan.clone() })?;
        for e in &plan.edits {
            match self.write_page(&e.path, &e.new_text, Some(&e.base_hash)) {
                Ok(_) => report.rewritten.push(e.path.clone()),
                Err(Error::Conflict { path }) => {
                    return Err(Error::PartialRename {
                        done: report.rewritten,
                        failed: path,
                    });
                }
                Err(other) => return Err(other),
            }
        }
        for (from, to) in &plan.file_moves {
            let (abs_from, abs_to) = (self.root().join(from), self.root().join(to));
            if abs_to.exists() {
                return Err(Error::Rename(format!("the file {to} already exists")));
            }
            if let Some(parent) = abs_to.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::rename(&abs_from, &abs_to)?;
            if let Some(parent) = abs_to.parent() {
                crate::sync_dir(parent);
            }
            // tidy: remove now-empty parent folders below pages/
            let mut dir = abs_from.parent().map(std::path::Path::to_path_buf);
            while let Some(d) = dir {
                if d == self.root().join("pages")
                    || d == self.root().join("journals")
                    || !d.starts_with(self.root())
                    || std::fs::remove_dir(&d).is_err()
                {
                    break;
                }
                dir = d.parent().map(std::path::Path::to_path_buf);
            }
            {
                let mut g = self.lock();
                let root = g.root.clone();
                g.index.reindex_file(&root, from)?;
                g.index.reindex_file(&root, to)?;
                g.remove_snapshot(from);
                g.move_versions(from, to);
                g.snapshot_file(to);
            }
            report.moved.push((from.clone(), to.clone()));
        }
        Ok(report)
    }
}

/// The patches that keep the relative link and image paths of a page file working when the file moves from
/// `from` to `to` (graph-relative paths): `../assets/x.png` in `pages/a.md` becomes `../../assets/x.png` in
/// `pages/ns/a.md`. Absolute paths, URLs, anchors and anything inside code are left alone.
fn relink_patches(text: &str, from: &str, to: &str) -> Vec<TextPatch> {
    let dir = |p: &str| -> Vec<String> {
        let mut v: Vec<String> = p.split('/').map(String::from).collect();
        v.pop();
        v
    };
    let (from_dir, to_dir) = (dir(from), dir(to));
    if from_dir == to_dir {
        return Vec::new();
    }
    let mut patches = Vec::new();
    let mut offset = 0;
    let mut fenced = false;
    for line in text.split_inclusive('\n') {
        let start = offset;
        offset += line.len();
        if line.trim_start().starts_with("```") || line.trim_start().starts_with("~~~") {
            fenced = !fenced;
            continue;
        }
        if fenced {
            continue;
        }
        let mut at = 0;
        while let Some(k) = line[at..].find("](") {
            let dest_start = at + k + 2;
            at = dest_start;
            // inside an inline code span: an odd number of backticks before
            if line[..dest_start].matches('`').count() % 2 == 1 {
                continue;
            }
            let dest_len = line[dest_start..]
                .find([')', ' ', '\t', '\n'])
                .unwrap_or(line.len() - dest_start);
            let dest = &line[dest_start..dest_start + dest_len];
            if let Some(new) = relinked(dest, &from_dir, &to_dir) {
                let range = notatnik_core::Span {
                    start: start + dest_start,
                    end: start + dest_start + dest_len,
                };
                patches.push(TextPatch {
                    range,
                    old_text: dest.to_string(),
                    new_text: new,
                });
            }
            at = dest_start + dest_len;
        }
    }
    patches
}

/// `dest` (a link target written in a file in `from_dir`) as it must be written in a file in `to_dir`; `None`
/// when it is not a relative path to somewhere inside the graph, or is the same.
fn relinked(dest: &str, from_dir: &[String], to_dir: &[String]) -> Option<String> {
    let first = dest.chars().next()?;
    let scheme = dest.split_once(':').is_some_and(|(s, _)| {
        s.chars()
            .all(|c| c.is_ascii_alphanumeric() || "+.-".contains(c))
    });
    if first == '#' || first == '/' || first == '?' || scheme {
        return None;
    }
    let cut = dest.find(['#', '?']).unwrap_or(dest.len());
    let (path, suffix) = dest.split_at(cut);
    let mut target: Vec<&str> = from_dir.iter().map(String::as_str).collect();
    for part in path.split('/') {
        match part {
            "." | "" => {}
            ".." => {
                target.pop()?;
            }
            p => target.push(p),
        }
    }
    let common = to_dir
        .iter()
        .zip(&target)
        .take_while(|(a, b)| a.as_str() == **b)
        .count();
    let mut out = "../".repeat(to_dir.len() - common);
    out.push_str(&target[common..].join("/"));
    out.push_str(suffix);
    (out != dest).then_some(out)
}
