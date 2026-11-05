//! Several graphs open in one app (MG-1). The first graph lives in the root `AppState`; every other open graph has a
//! state of its own in `slots`, so keys, sync, undo and the index of one graph never meet those of another. The root
//! routes the commands that name a slot (`dispatch_slot`) and answers the ones that involve several graphs.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::{Value, json};

use super::{ApiError, ApiResult, AppState, Emitter};
use crate::dto::{NoArgs, SlotArgs, SlotInfo};

/// An emitter that adds `slot` to every event, so the UI can tell which graph it came from.
fn tagged(emit: Emitter, slot: &str) -> Emitter {
    let slot = slot.to_string();
    Arc::new(move |name, mut payload| {
        if let Value::Object(m) = &mut payload {
            m.insert("slot".into(), json!(slot));
        }
        emit(name, payload);
    })
}

/// A slot name is short and plain: it is in URLs and file names of the UI.
fn valid_slot(slot: &str) -> bool {
    (1..=8).contains(&slot.len()) && slot.bytes().all(|b| b.is_ascii_alphanumeric())
}

impl AppState {
    /// The state of an extra slot, made when it is first named. Only the root has slots.
    pub fn slot_state(&self, slot: &str) -> ApiResult<Arc<AppState>> {
        if !valid_slot(slot) {
            return Err(ApiError::new(
                "invalid",
                format!("`{slot}` is not a slot name"),
            ));
        }
        if !self.slot.is_empty() {
            return Err(ApiError::new("invalid", "only the first state has slots"));
        }
        let mut slots = self
            .slots
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(s) = slots.get(slot) {
            return Ok(s.clone());
        }
        let mut s = AppState::new(super::AppOptions {
            dirs: self.dirs.clone(),
            emit: tagged(self.emit.clone(), slot),
            today: self.today.clone(),
            watch: self.watch,
        });
        s.secrets = self.secrets.clone();
        s.calendar_client = self.calendar_client.clone();
        s.backup_keys = self.backup_keys.clone();
        s.open_roots = self.open_roots.clone();
        s.clip_policy = self.clip_policy.clone();
        s.slot = slot.to_string();
        let s = Arc::new(s);
        slots.insert(slot.to_string(), s.clone());
        Ok(s)
    }

    /// The state for a slot name that may be empty (the root itself, as `None`).
    pub(super) fn existing_slot(&self, slot: &str) -> Option<Arc<AppState>> {
        self.slots
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(slot)
            .cloned()
    }

    /// `open_graph` calls this first: the same folder is never open in two slots.
    pub(super) fn refuse_if_open_elsewhere(&self, path: &Path) -> ApiResult<()> {
        let want = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        let roots = self
            .open_roots
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let other = roots
            .iter()
            .find(|(slot, root)| **slot != self.slot && **root == want);
        match other {
            Some(_) => Err(ApiError::new(
                "already_open",
                "this graph is already open in the other pane",
            )),
            None => Ok(()),
        }
    }

    /// Remembers which folder this state has open.
    pub(super) fn note_open_root(&self, graph: &notatnik_graph::Graph) {
        let root =
            std::fs::canonicalize(graph.root()).unwrap_or_else(|_| graph.root().to_path_buf());
        self.open_roots
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(self.slot.clone(), root);
    }

    fn slot_info(&self) -> Option<SlotInfo> {
        self.with_open(|o| {
            let root: PathBuf = o.graph.root().to_path_buf();
            Ok(SlotInfo {
                slot: self.slot.clone(),
                graph_id: o.graph.id(),
                name: root
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                root: root.to_string_lossy().into_owned(),
                color: self.graph_color(&o.graph.id()),
            })
        })
        .ok()
    }

    /// `slots`: the open graphs, the first one first.
    pub fn slots_cmd(&self, _a: NoArgs) -> ApiResult<Vec<SlotInfo>> {
        let mut out: Vec<SlotInfo> = self.slot_info().into_iter().collect();
        let mut others: Vec<Arc<AppState>> = self
            .slots
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .values()
            .cloned()
            .collect();
        others.sort_by(|a, b| a.slot.cmp(&b.slot));
        out.extend(others.iter().filter_map(|s| s.slot_info()));
        Ok(out)
    }

    /// `close_slot`: closes the graph of an extra slot. It gets a last sync and snapshot (on a thread, so a slow
    /// server does not hold the window), and its folder can be opened again anywhere.
    pub fn close_slot(&self, a: SlotArgs) -> ApiResult<Vec<SlotInfo>> {
        if a.slot.is_empty() {
            return Err(ApiError::new(
                "invalid",
                "the first graph is not closed; open another graph in its place",
            ));
        }
        let removed = self
            .slots
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&a.slot);
        self.open_roots
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&a.slot);
        if let Some(s) = removed {
            log::info!("closed the graph in slot {}", a.slot);
            std::thread::spawn(move || {
                s.sync_on_quit_own();
                s.backup_on_quit_own();
            });
        }
        self.slots_cmd(NoArgs {})
    }

    /// The states of all extra slots.
    pub(super) fn other_states(&self) -> Vec<Arc<AppState>> {
        self.slots
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .values()
            .cloned()
            .collect()
    }

    /// `read_asset` for the graph in a slot (`""` is the first graph).
    pub fn read_asset_in(&self, slot: &str, rel: &str) -> ApiResult<Vec<u8>> {
        if slot.is_empty() {
            return self.read_asset(rel);
        }
        self.existing_slot(slot)
            .ok_or_else(|| ApiError::new("no_graph", "no graph is open in that slot"))?
            .read_asset(rel)
    }

    /// `asset_file` for the graph in a slot (`""` is the first graph).
    pub fn asset_file_in(&self, slot: &str, rel: &str) -> ApiResult<PathBuf> {
        if slot.is_empty() {
            return self.asset_file(rel);
        }
        self.existing_slot(slot)
            .ok_or_else(|| ApiError::new("no_graph", "no graph is open in that slot"))?
            .asset_file(rel)
    }
}
