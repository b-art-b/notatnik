//! Encrypting chosen paths on the graph's WebDAV server (EN-11 to EN-18): the graph's vault, the key in memory or in
//! the keyring, the list of marked paths, and what marking them means. The encrypting itself is in `notatnik-sync`.

use notatnik_core::encrypted::{EncryptedPaths, check_path};
use notatnik_crypt::{KdfParams, Keys, Vault};

use super::*;

/// The key of the graph in the system keyring (EN-14).
fn keyring_key(graph: &str) -> String {
    format!("graph key {graph}")
}

/// A vault that was made and whose recovery key was shown but not confirmed yet. It lives in memory only: closing
/// the app before the confirmation makes nothing.
pub(super) struct PendingVault {
    graph: String,
    vault: Vault,
    keys: Arc<Keys>,
}

/// The unlocked key of the open graph, shared with the sync thread.
pub(super) type SharedKeys = Arc<Mutex<Option<Arc<Keys>>>>;

fn crypt_err(e: &notatnik_crypt::Error) -> ApiError {
    match e {
        notatnik_crypt::Error::WrongSecret => ApiError::new(
            "invalid",
            "that is not the password or the recovery key of this graph",
        ),
        e => ApiError::new("invalid", e.to_string()),
    }
}

/// The paths that hold the files of a page: its file, and with `subpages` its folder.
fn page_paths(rel: &str, subpages: bool) -> Vec<String> {
    let mut out = vec![rel.to_string()];
    if subpages {
        if let Some(stem) = rel.strip_suffix(".md") {
            out.push(format!("{stem}/"));
        }
    }
    out
}

fn covers(rule: &str, path: &str) -> bool {
    if rule.ends_with('/') {
        path.starts_with(rule)
    } else {
        rule == path
    }
}

impl AppState {
    /// Forgets the keys (another graph is opened, or the user locks).
    pub(super) fn set_graph_keys(&self, keys: Option<Arc<Keys>>) {
        if let Ok(mut g) = self.graph_keys.lock() {
            *g = keys;
        }
    }

    fn graph_keys_now(&self) -> Option<Arc<Keys>> {
        self.graph_keys.lock().ok().and_then(|g| g.clone())
    }

    /// When a graph opens: the key from the keyring, if the switch is on and the key still fits the vault.
    pub(super) fn unlock_from_keyring(&self, graph: &Graph) {
        self.set_graph_keys(None);
        if let Ok(mut p) = self.graph_pending.lock() {
            *p = None;
        }
        let Ok(Some(vault)) = graph.read_vault() else {
            return;
        };
        let Some(text) = self.secrets.get(&keyring_key(&graph.id())) else {
            return;
        };
        match vault.unlock_exported(&text) {
            Ok(keys) => {
                log::info!("encryption: the key of the graph was taken from the keyring");
                self.set_graph_keys(Some(Arc::new(keys)));
            }
            Err(e) => log::warn!("encryption: the key in the keyring does not fit the vault: {e}"),
        }
    }

    /// Removes what this device keeps of a graph's key.
    pub(super) fn forget_graph_key(&self, graph: &str) {
        self.secrets.delete(&keyring_key(graph));
    }

    fn encryption_status_of(&self, o: &Open) -> EncryptionStatus {
        let cfg = o.graph.config().encryption;
        let vault = o.graph.read_vault().ok().flatten();
        let id = o.graph.id();
        let dev = notatnik_graph::DeviceSettings::load(&self.dirs).unwrap_or_default();
        let sync = o.sync.as_ref().map(|t| t.status());
        EncryptionStatus {
            configured: vault.is_some(),
            pending: self
                .graph_pending
                .lock()
                .is_ok_and(|p| p.as_ref().is_some_and(|p| p.graph == id)),
            unlocked: self.graph_keys_now().is_some(),
            key_id: vault.map(|v| v.key_id).unwrap_or_default(),
            keyring: self.secrets.get(&keyring_key(&id)).is_some(),
            has_server: sync.is_some(),
            backups_plain: !cfg.paths.is_empty() && !dev.ui.backup_encrypt,
            paths: cfg.paths,
            calendar: cfg.calendar,
            left_alone: sync.as_ref().map_or(0, |s| s.left_alone),
            reserved: sync.map(|s| s.reserved).unwrap_or_default(),
        }
    }

    fn status_now(&self) -> ApiResult<EncryptionStatus> {
        self.with_open(|o| Ok(self.encryption_status_of(o)))
    }

    /// Asks the sync for a run soon: a key that was unlocked, a path that was marked.
    fn kick_sync(&self, secs: u64) {
        let _ = self.with_open(|o| {
            if let Some(t) = &o.sync {
                t.kick(secs);
            }
            Ok(())
        });
    }

    /// `encryption_status`
    pub fn graph_encryption_status(&self, _a: NoArgs) -> ApiResult<EncryptionStatus> {
        self.status_now()
    }

    /// `encryption_setup`: makes the graph's vault and returns its recovery key, to be shown once. The vault is
    /// written to the graph (`.notatnik/vault.json`) only with `encryption_confirm`.
    pub fn encryption_setup(&self, a: EncryptionSetupArgs) -> ApiResult<RecoveryKeyResult> {
        Self::check_new_password(&a.password)?;
        let id = self.with_open(|o| {
            if o.graph.read_vault()?.is_some() {
                return Err(ApiError::new(
                    "invalid",
                    "this graph has encryption already: unlock it, or change the password",
                ));
            }
            Ok(o.graph.id())
        })?;
        let (vault, keys, recovery) = Vault::create(&a.password, KdfParams::default())
            .map_err(|e| ApiError::new("internal", e.to_string()))?;
        *self
            .graph_pending
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(PendingVault {
            graph: id,
            vault,
            keys: Arc::new(keys),
        });
        Ok(RecoveryKeyResult {
            recovery_key: recovery.to_string(),
        })
    }

    /// `encryption_confirm`: the recovery key is saved; the vault is written to the graph and its key is unlocked.
    pub fn encryption_confirm(&self, _a: NoArgs) -> ApiResult<EncryptionStatus> {
        let pending = self
            .graph_pending
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        let Some(p) = pending else {
            return Err(ApiError::new(
                "invalid",
                "there is no encryption to confirm",
            ));
        };
        let status = self.with_open(|o| {
            if o.graph.id() != p.graph {
                return Err(ApiError::new(
                    "invalid",
                    "the encryption was set up for another graph",
                ));
            }
            o.graph.write_vault(&p.vault)?;
            log::info!(
                "encryption: the vault of the graph was made (key {})",
                p.vault.key_id
            );
            Ok(())
        });
        status?;
        self.set_graph_keys(Some(p.keys));
        self.kick_sync(1);
        self.status_now()
    }

    fn vault_of(o: &Open) -> ApiResult<Vault> {
        o.graph
            .read_vault()?
            .ok_or_else(|| ApiError::new("invalid", "this graph has no encryption set up"))
    }

    /// `encryption_unlock`: the password or the recovery key; the key stays in memory until locked or the app ends.
    pub fn encryption_unlock(&self, a: EncryptionUnlockArgs) -> ApiResult<EncryptionStatus> {
        let vault = self.with_open(|o| Self::vault_of(o))?;
        let keys = vault.unlock(&a.secret).map_err(|e| crypt_err(&e))?;
        self.set_graph_keys(Some(Arc::new(keys)));
        log::info!("encryption: unlocked");
        self.kick_sync(1);
        self.status_now()
    }

    /// `encryption_lock`: forgets the key. Encrypted files are left alone by the sync until it is unlocked again.
    pub fn encryption_lock(&self, _a: NoArgs) -> ApiResult<EncryptionStatus> {
        self.set_graph_keys(None);
        log::info!("encryption: locked");
        self.status_now()
    }

    /// `encryption_change_password`: rewraps the key; no file is encrypted again.
    pub fn encryption_change_password(
        &self,
        a: EncryptionChangePasswordArgs,
    ) -> ApiResult<EncryptionStatus> {
        Self::check_new_password(&a.new_password)?;
        self.with_open(|o| {
            let mut vault = Self::vault_of(o)?;
            let keys = vault.unlock(&a.old_password).map_err(|e| crypt_err(&e))?;
            vault
                .change_password(&keys, &a.new_password, KdfParams::default())
                .map_err(|e| ApiError::new("internal", e.to_string()))?;
            o.graph.write_vault(&vault)?;
            Ok(())
        })?;
        self.kick_sync(1);
        self.status_now()
    }

    /// `encryption_new_recovery_key`: the old recovery key stops working; the new one is shown once.
    pub fn encryption_new_recovery_key(
        &self,
        a: EncryptionPasswordArgs,
    ) -> ApiResult<RecoveryKeyResult> {
        let recovery = self.with_open(|o| {
            let mut vault = Self::vault_of(o)?;
            let keys = vault.unlock(&a.password).map_err(|e| crypt_err(&e))?;
            let recovery = vault
                .new_recovery_key(&keys)
                .map_err(|e| ApiError::new("internal", e.to_string()))?;
            o.graph.write_vault(&vault)?;
            Ok(recovery)
        })?;
        self.kick_sync(1);
        Ok(RecoveryKeyResult {
            recovery_key: recovery.to_string(),
        })
    }

    /// `encryption_keyring`: keeps the key in the system keyring so the app unlocks itself, or removes it there.
    pub fn encryption_keyring(&self, a: EncryptionKeyringArgs) -> ApiResult<EncryptionStatus> {
        let id = self.with_open(|o| Ok(o.graph.id()))?;
        if a.on {
            let keys = self.graph_keys_now().ok_or_else(|| {
                ApiError::new("locked", "unlock the encryption first, then keep the key")
            })?;
            self.secrets
                .set(&keyring_key(&id), &keys.export_hex())
                .map_err(|e| {
                    ApiError::new(
                        "invalid",
                        format!("the key could not be stored in the keyring: {e}"),
                    )
                })?;
            log::info!("encryption: the key is kept in the keyring");
        } else {
            self.secrets.delete(&keyring_key(&id));
            log::info!("encryption: the key was removed from the keyring");
        }
        self.status_now()
    }

    /// `encryption_calendar`: whether the tasks of encrypted pages go to the calendar.
    pub fn encryption_calendar(&self, a: EncryptionCalendarArgs) -> ApiResult<EncryptionStatus> {
        self.with_open(|o| {
            o.graph.update_config(|c| {
                c.encryption.calendar = if a.allow { "allow" } else { "exclude" }.into();
            })?;
            Ok(())
        })?;
        self.status_now()
    }

    /// `encryption_page_paths`: the paths to mark for a page, and with `subpages` for the pages below it.
    pub fn encryption_page_paths(&self, a: EncryptionPagePathsArgs) -> ApiResult<StringList> {
        self.with_open(|o| {
            let rel = Self::page_rel(o, &a.page)?;
            Ok(StringList {
                items: page_paths(&rel, a.subpages),
            })
        })
    }

    /// The files of the graph (relative paths) under the given paths: pages that are indexed and files that exist.
    fn files_under(o: &Open, paths: &[String]) -> Vec<String> {
        let rules = EncryptedPaths::new(paths);
        let mut out: Vec<String> = o
            .graph
            .with_index(|i| i.page_files())
            .map(|f| f.into_iter().map(|(_, rel)| rel).collect())
            .unwrap_or_default();
        out.retain(|rel| rules.is_encrypted(rel));
        out.sort();
        out.dedup();
        out
    }

    /// Pictures named in the text of `rels` that are not stored encrypted.
    fn images_of(o: &Open, rels: &[String], rules: &EncryptedPaths) -> Vec<String> {
        let mut found = std::collections::BTreeSet::new();
        for rel in rels {
            let Ok(file) = o.graph.read_page(rel) else {
                continue;
            };
            let mut rest = file.text.as_str();
            while let Some(i) = rest.find("assets/") {
                let tail = &rest[i..];
                let end = tail
                    .find([')', ']', '"', '\'', ' ', '\n', '<', '>'])
                    .unwrap_or(tail.len());
                let path = tail[..end].to_string();
                if path.len() > "assets/".len() && !rules.is_encrypted(&path) {
                    found.insert(path);
                }
                rest = &tail[end..];
            }
        }
        found.into_iter().collect()
    }

    /// `encryption_preview`: what marking these paths means, before it is done (EN-16, EN-17).
    pub fn encryption_preview(&self, a: EncryptionPathsArgs) -> ApiResult<EncryptionPreview> {
        for p in &a.paths {
            check_path(p).map_err(|why| ApiError::new("invalid", format!("`{p}` {why}")))?;
        }
        self.with_open(|o| {
            let rules = EncryptedPaths::new(&a.paths);
            let files = Self::files_under(o, &a.paths);
            let synced_before = o.sync.as_ref().is_some_and(|_| {
                notatnik_sync::SyncState::load(&syncing::state_file(o.graph.root()))
                    .files
                    .iter()
                    .any(|(p, f)| rules.is_encrypted(p) && !f.enc)
            });
            let mut all_rules = o.graph.config().encryption.paths;
            all_rules.extend(a.paths.iter().cloned());
            let images = Self::images_of(o, &files, &EncryptedPaths::new(&all_rules));
            let calendar_tasks = files
                .iter()
                .filter_map(|rel| o.graph.read_page(rel).ok())
                .map(|f| {
                    f.text
                        .lines()
                        .filter(|l| {
                            let t = l.trim_start().trim_start_matches("- ");
                            matches!(
                                t.split_whitespace().next(),
                                Some("TODO" | "DOING" | "WAITING" | "LATER" | "NOW")
                            )
                        })
                        .count() as u32
                })
                .sum();
            Ok(EncryptionPreview {
                synced_before,
                files: files.len() as u32,
                images,
                calendar_tasks,
            })
        })
    }

    /// `encryption_mark`: adds paths to the encrypted ones, or removes them. The key must be unlocked: files are
    /// stored again in the other form at the next sync, and that needs it.
    pub fn encryption_mark(&self, a: EncryptionPathsArgs) -> ApiResult<EncryptionStatus> {
        for p in &a.paths {
            check_path(p).map_err(|why| ApiError::new("invalid", format!("`{p}` {why}")))?;
        }
        if self.graph_keys_now().is_none() {
            let configured = self.with_open(|o| Ok(o.graph.read_vault()?.is_some()))?;
            return Err(if configured {
                ApiError::new("locked", "unlock the encryption first")
            } else {
                ApiError::new("invalid", "set up encryption for this graph first")
            });
        }
        self.with_open(|o| {
            let current = o.graph.config().encryption.paths;
            let mut next = current.clone();
            for p in &a.paths {
                if a.encrypted {
                    if !next.contains(p) {
                        next.push(p.clone());
                    }
                } else if let Some(i) = next.iter().position(|x| x == p) {
                    next.remove(i);
                } else if let Some(cover) = next.iter().find(|r| covers(r, p)) {
                    return Err(ApiError::new(
                        "invalid",
                        format!("`{p}` is encrypted because `{cover}` is; stop encrypting that one instead"),
                    ));
                }
            }
            if next != current {
                o.graph.update_config(|c| c.encryption.paths = next.clone())?;
                log::info!(
                    "encryption: {} {}",
                    if a.encrypted { "now encrypted:" } else { "no longer encrypted:" },
                    a.paths.join(", ")
                );
            }
            Ok(())
        })?;
        self.kick_sync(1);
        self.status_now()
    }
}
