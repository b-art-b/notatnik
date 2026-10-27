//! Hiding pages (NS-20, NS-27). A page that has a file is hidden by `archived: true` in its front matter; a name without
//! a file (a folder that exists because a page below it does) by an entry of `hidden = [...]` in `.notatnik/config.toml`.
//! Either mark hides the page and everything below it. `hide_pages` marks or unmarks many names at once.

use super::favorites::renamed_list;
use super::*;

/// The name (of `marks`) that hides `name`: the name itself, or the folder above it.
fn covered_by(marks: &[String], name: &str) -> Option<String> {
    let key = PageName::parse(name).map_or_else(|_| name.to_lowercase(), |n| n.lookup_key());
    marks
        .iter()
        .find(|m| {
            let mk = PageName::parse(m).map_or_else(|_| m.to_lowercase(), |n| n.lookup_key());
            key == mk || key.starts_with(&format!("{mk}/"))
        })
        .cloned()
}

fn is_itself(marks: &[String], name: &str) -> bool {
    covered_by(marks, name).is_some_and(|m| {
        PageName::parse(&m).map(|a| a.lookup_key()).ok()
            == PageName::parse(name).map(|b| b.lookup_key()).ok()
    })
}

impl AppState {
    /// Every name that carries a hide mark: the front matter of a page with a file, the `hidden` list for the others.
    pub(super) fn hidden_names(o: &Open) -> ApiResult<Vec<String>> {
        let mut names = o.graph.with_index(|i| i.archived_pages())?;
        for h in o.graph.config().hidden {
            if !names.iter().any(|n| n.eq_ignore_ascii_case(&h)) {
                names.push(h);
            }
        }
        Ok(names)
    }

    /// `hide_pages`: hides or unhides the given pages (each with what is below it). A name with a file gets or loses
    /// `archived: true`, a name without one goes into or out of the `hidden` list. A name below another name of the
    /// same request is left to that one. A page that is hidden only by a folder above it cannot be unhidden alone: it is
    /// listed in `inherited` with the folder.
    pub fn hide_pages(&self, a: HidePagesArgs) -> ApiResult<HideResult> {
        let mut names: Vec<PageName> = Vec::new();
        for p in &a.pages {
            let n = Self::parse_name(p)?;
            if !names.iter().any(|x| x.lookup_key() == n.lookup_key()) {
                names.push(n);
            }
        }
        // the one above covers the ones below it
        let keys: Vec<String> = names.iter().map(PageName::lookup_key).collect();
        names.retain(|n| {
            let k = n.lookup_key();
            !keys.iter().any(|o| k.starts_with(&format!("{o}/")))
        });
        self.with_open(|o| {
            let marks = Self::hidden_names(o)?;
            let mut changed: Vec<String> = Vec::new();
            let mut paths: Vec<String> = Vec::new();
            let mut inherited: Vec<InheritedHide> = Vec::new();
            let mut list_add: Vec<String> = Vec::new();
            let mut list_drop: Vec<String> = Vec::new();
            for n in &names {
                let info = o.graph.with_index(|i| i.page_info(n.as_str()))?;
                let canonical = info
                    .as_ref()
                    .map_or_else(|| n.to_string(), |i| i.name.clone());
                let file = info
                    .as_ref()
                    .filter(|i| !i.is_virtual)
                    .and_then(|i| i.file_path.clone());
                let in_list = o
                    .graph
                    .config()
                    .hidden
                    .iter()
                    .any(|h| same_name(h, &canonical));
                let covered = covered_by(&marks, &canonical);
                if a.hidden {
                    if covered.is_some() {
                        continue; // hidden already, by itself or by a folder above
                    }
                    match file {
                        Some(rel) => {
                            Self::set_archived(o, &rel, true)?;
                            paths.push(rel);
                        }
                        None => list_add.push(canonical.clone()),
                    }
                    changed.push(canonical);
                } else {
                    let itself = is_itself(&marks, &canonical);
                    if !itself {
                        if let Some(by) = covered {
                            inherited.push(InheritedHide {
                                name: canonical,
                                by,
                            });
                        }
                        continue;
                    }
                    if let Some(rel) = file {
                        if Self::set_archived(o, &rel, false)? {
                            paths.push(rel);
                        }
                    }
                    if in_list {
                        list_drop.push(canonical.clone());
                    }
                    changed.push(canonical);
                }
            }
            if !list_add.is_empty() || !list_drop.is_empty() {
                o.graph.update_config(|c| {
                    c.hidden
                        .retain(|h| !list_drop.iter().any(|d| same_name(h, d)));
                    for n in &list_add {
                        if !c.hidden.iter().any(|h| same_name(h, n)) {
                            c.hidden.push(n.clone());
                        }
                    }
                })?;
            }
            log::info!(
                "{} {} page(s): {}",
                if a.hidden { "hid" } else { "unhid" },
                changed.len(),
                changed.join(", ")
            );
            (self.emit)(
                "graph-changed",
                json!({ "pages": changed.clone(), "paths": paths, "own": false }),
            );
            Ok(HideResult { changed, inherited })
        })
    }

    /// Puts `archived: true` into the file `rel`, or takes the line out. Returns whether the file changed.
    fn set_archived(o: &Open, rel: &str, on: bool) -> ApiResult<bool> {
        let file = o.graph.read_page(rel)?;
        let has = notatnik_core::parse_page(&file.text)
            .frontmatter
            .is_some_and(|f| f.entries.iter().any(|e| e.key == "archived"));
        let op = match (on, has) {
            (true, _) => BlockOp::SetPageProperty {
                key: "archived".into(),
                value: "true".into(),
            },
            (false, true) => BlockOp::RemovePageProperty {
                key: "archived".into(),
            },
            (false, false) => return Ok(false),
        };
        o.graph.edit_page(rel, Some(&file.hash), &[op])?;
        Ok(true)
    }

    /// After a rename: the hidden names follow the page, and the ones below it when they moved with it.
    pub(super) fn rename_hidden(o: &Open, old: &str, new: &str, subpages: bool) {
        let list = o.graph.config().hidden;
        let renamed = renamed_list(&list, old, new, subpages);
        if renamed != list {
            if let Err(e) = o.graph.update_config(|c| c.hidden = renamed) {
                log::warn!("hidden: could not follow a rename: {e}");
            }
        }
    }

    /// After a delete: a deleted page is no entry of the list any more.
    pub(super) fn remove_hidden(o: &Open, names: &[String]) {
        let list = o.graph.config().hidden;
        let kept: Vec<String> = list
            .iter()
            .filter(|h| !names.iter().any(|n| same_name(h, n)))
            .cloned()
            .collect();
        if kept.len() != list.len() {
            if let Err(e) = o.graph.update_config(|c| c.hidden = kept) {
                log::warn!("hidden: could not drop a deleted page: {e}");
            }
        }
    }
}

fn same_name(a: &str, b: &str) -> bool {
    PageName::parse(a)
        .and_then(|a| PageName::parse(b).map(|b| a.lookup_key() == b.lookup_key()))
        .unwrap_or(a == b)
}
