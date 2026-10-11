//! The operation journal (WAL for multi-file operations): a rename or a delete of several pages is several
//! atomic file operations. Before the first one a record of the whole operation is written to
//! `<data dir>/journal/pending.json`; it is removed when the operation returns, successfully or not. A record that is
//! still there when a graph opens means the app died in the middle: the operation is undone (or, if it had in fact
//! completed, only the record is dropped) and the user is told.
//!
//! The journal lives outside the graph folder (ST-2); a crash leaves at worst a stale record, never a lost file,
//! because every step it protects is itself atomic and the copies it needs are written before it is.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::rename::RenamePlan;
use crate::{Graph, Result, sync_dir};

/// What was in progress.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub(crate) enum Pending {
    /// A rename or namespace move: rewrites first, file moves last.
    Rename { plan: RenamePlan },
    /// Pages moved to the trash entry `trash_id` (their files are in it before the first one is deleted).
    Delete { trash_id: String, rels: Vec<String> },
}

/// Removes the record when the operation returns. A crash never runs this, which is the point.
pub(crate) struct Journaled<'a> {
    graph: &'a Graph,
}

impl Drop for Journaled<'_> {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(self.graph.journal_path());
    }
}

impl Graph {
    fn journal_path(&self) -> PathBuf {
        self.lock().data_dir.join("journal").join("pending.json")
    }

    /// Writes the record durably, then returns the guard that removes it.
    pub(crate) fn journal_begin(&self, op: &Pending) -> Result<Journaled<'_>> {
        let path = self.journal_path();
        let dir = path.parent().map(std::path::Path::to_path_buf);
        if let Some(d) = &dir {
            std::fs::create_dir_all(d)?;
        }
        let tmp = path.with_extension("tmp");
        let text = serde_json::to_vec(op).map_err(|e| std::io::Error::other(e.to_string()))?;
        {
            use std::io::Write;
            let mut f = std::fs::File::create(&tmp)?;
            f.write_all(&text)?;
            f.sync_all()?;
        }
        std::fs::rename(&tmp, &path)?;
        if let Some(d) = &dir {
            sync_dir(d);
        }
        Ok(Journaled { graph: self })
    }

    /// Called when a graph opens: undoes what an interrupted operation left. Returns a message for the user
    /// when something was undone (or could not be).
    pub(crate) fn journal_recover(&self) -> Option<String> {
        let path = self.journal_path();
        let text = std::fs::read(&path).ok()?;
        let Ok(op) = serde_json::from_slice::<Pending>(&text) else {
            // unreadable (a torn write of the record itself): nothing it protected has started
            let _ = std::fs::remove_file(&path);
            return None;
        };
        let root = self.root().to_path_buf();
        let message = match op {
            Pending::Rename { plan } => {
                let moved = plan
                    .file_moves
                    .iter()
                    .all(|(f, t)| root.join(t).exists() && !root.join(f).exists());
                let rewritten = plan.edits.iter().all(|e| {
                    std::fs::read_to_string(root.join(&e.path)).is_ok_and(|t| t == e.new_text)
                });
                if moved && rewritten {
                    None // it had completed
                } else {
                    Some(match self.revert_rename(&plan) {
                        Ok(files) => format!(
                            "The app stopped in the middle of renaming `{}` to `{}`. The rename was undone ({} file(s) put back).",
                            plan.old,
                            plan.new,
                            files.len()
                        ),
                        Err(e) => format!(
                            "The app stopped in the middle of renaming `{}` to `{}` and it could not be undone: {e}",
                            plan.old, plan.new
                        ),
                    })
                }
            }
            Pending::Delete { trash_id, rels } => {
                if rels.iter().all(|r| !root.join(r).exists()) {
                    None // every file had been moved to the trash
                } else {
                    Some(match self.restore_trash(&trash_id) {
                        Ok(files) => format!(
                            "The app stopped in the middle of deleting {} page(s). The deletion was undone ({} file(s) put back).",
                            rels.len(),
                            files.len()
                        ),
                        Err(e) => format!(
                            "The app stopped in the middle of deleting pages and it could not be undone: {e}. The files are in the trash."
                        ),
                    })
                }
            }
        };
        let _ = std::fs::remove_file(&path);
        message
    }
}
