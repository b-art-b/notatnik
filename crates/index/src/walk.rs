//! Finding the Markdown files of a graph on disk.

use std::path::{Path, PathBuf};

/// One page file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphFile {
    /// Path relative to the graph root, `/`-separated.
    pub rel: String,
    /// Absolute path.
    pub abs: PathBuf,
}

/// Result of scanning a graph folder.
#[derive(Debug, Clone, Default)]
pub struct GraphFiles {
    /// Files that map to a page, sorted by path.
    pub pages: Vec<GraphFile>,
    /// Nextcloud conflict copies (not indexed as pages).
    pub conflicts: Vec<String>,
    /// `.md` files under `pages/` or `journals/` that don't map to a page name.
    pub unmapped: Vec<String>,
}

/// True for Nextcloud conflict file names like `alpha (conflicted copy 2026-10-03 120000).md`.
pub fn is_conflict_name(name: &str) -> bool {
    name.contains("(conflicted copy ")
}

fn walk_dir(dir: &Path, root: &Path, out: &mut GraphFiles) -> std::io::Result<()> {
    let mut entries: Vec<_> = std::fs::read_dir(dir)?.collect::<std::io::Result<Vec<_>>>()?;
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let name = e.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        let path = e.path();
        let ft = e.file_type()?;
        if ft.is_dir() {
            walk_dir(&path, root, out)?;
        } else if ft.is_file() && name.ends_with(".md") {
            let rel = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            if is_conflict_name(&name) {
                out.conflicts.push(rel);
            } else if notatnik_core::path_to_page(&rel).is_some() {
                out.pages.push(GraphFile { rel, abs: path });
            } else {
                out.unmapped.push(rel);
            }
        }
    }
    Ok(())
}

/// Scans `pages/` and `journals/` below `root`.
pub fn scan(root: &Path) -> std::io::Result<GraphFiles> {
    let mut out = GraphFiles::default();
    for sub in ["journals", "pages"] {
        let dir = root.join(sub);
        if dir.is_dir() {
            walk_dir(&dir, root, &mut out)?;
        }
    }
    out.pages.sort_by(|a, b| a.rel.cmp(&b.rel));
    out.conflicts.sort();
    out.unmapped.sort();
    Ok(out)
}
