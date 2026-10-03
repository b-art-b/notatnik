//! Renaming a page with reference rewriting (P1.34, LN-7).

use notatnik_core::{
    Block, PageName, ParseOptions, RefKind, TextPatch, apply_patches, page_to_path, parse_page_with,
};

use crate::{Error, Graph, Result};

/// The rewrite of one file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileEdit {
    /// Graph-relative path.
    pub path: String,
    /// Hash of the file content the edit was computed from (guard).
    pub base_hash: String,
    /// The text after the rewrite.
    pub new_text: String,
    /// The individual replacements (for previews).
    pub patches: Vec<TextPatch>,
}

/// Everything a rename would do; produced by [`Graph::plan_rename`] and applied by [`Graph::apply_rename`].
#[derive(Debug, Clone, PartialEq, Eq)]
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

fn collect(b: &Block, source: &str, old_key: &str, new: &PageName, out: &mut Vec<TextPatch>) {
    for r in &b.refs {
        if !matches!(r.kind, RefKind::Page | RefKind::Tag | RefKind::PageEmbed) {
            continue;
        }
        if PageName::parse(&r.target)
            .map(|n| n.lookup_key())
            .as_deref()
            != Ok(old_key)
        {
            continue;
        }
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
        collect(c, source, old_key, new, out);
    }
}

impl Graph {
    /// Computes a rename without touching anything (preview, LN-7).
    pub fn plan_rename(&self, old: &str, new: &str) -> Result<RenamePlan> {
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
        let (info, children, referencing) = self.with_index(|i| {
            Ok::<_, Error>((
                i.page_info(old)?,
                i.namespace_children(old)?,
                i.files_referencing(old)?,
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
        if let Some(existing) = self.with_index(|i| i.page_info(new))? {
            if !existing.is_virtual {
                return Err(Error::Rename(format!(
                    "a page named `{}` already exists",
                    existing.name
                )));
            }
        }
        if !children.is_empty() {
            return Err(Error::Rename(format!(
                "`{old}` has subpages ({}); moving a whole namespace is not supported yet (NS-8)",
                children.join(", ")
            )));
        }
        let mut file_moves = Vec::new();
        if let Some(from) = &info.file_path {
            let to = page_to_path(&new_name);
            if self.root().join(&to).exists() {
                return Err(Error::Rename(format!("the file {to} already exists")));
            }
            if *from != to {
                file_moves.push((from.clone(), to));
            }
        }
        let opts = ParseOptions {
            property_types: self.config().property_types(),
        };
        let mut edits = Vec::new();
        let mut warnings = Vec::new();
        for path in referencing {
            let file = self.read_page(&path)?;
            let page = parse_page_with(&file.text, &opts);
            let mut patches = Vec::new();
            if let Some(p) = &page.preamble {
                collect(p, &file.text, &old_key, &new_name, &mut patches);
            }
            for b in &page.blocks {
                collect(b, &file.text, &old_key, &new_name, &mut patches);
            }
            if let Some(fm) = page.frontmatter.as_ref().filter(|f| f.valid) {
                let mentions = fm.tags.iter().any(|t| {
                    PageName::parse(t).map(|n| n.lookup_key()).as_deref() == Ok(old_key.as_str())
                });
                if mentions {
                    warnings.push(format!(
                        "{path}: frontmatter tags still mention `{old}`; edit them by hand"
                    ));
                }
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
                new_text,
                patches,
            });
        }
        Ok(RenamePlan {
            old: old_name.to_string(),
            new: new_name.to_string(),
            file_moves,
            edits,
            warnings,
        })
    }

    /// Applies a plan: rewrites references first (each file with its base-hash guard), moves files last.
    /// If a file conflicts, stops and reports what was already done; nothing is moved in that case.
    pub fn apply_rename(&self, plan: &RenamePlan) -> Result<RenameReport> {
        let mut report = RenameReport::default();
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
                g.snapshot_file(to);
            }
            report.moved.push((from.clone(), to.clone()));
        }
        Ok(report)
    }
}
