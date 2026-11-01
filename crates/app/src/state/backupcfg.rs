//! The backup settings of one graph on this device (BK-2, BK-3, BK-7, BK-9, EN-5): its snapshot folder, second
//! folder, automatic backups, compression and encryption, and its own backup vault with its own password and
//! recovery key. They live in `<config dir>/graphs/<graph id>/` (never in the graph, never synced; not in the
//! cache, which may be deleted: the vault holds keys).
//!
//! A graph without its own settings follows the device-wide values of `settings.toml` (what every graph used
//! before settings were per graph); the first change of one of them writes all five for the graph, and from then on
//! it is independent. The same for the vault: a graph without its own reads the old device-wide
//! `backup-vault.json`, so snapshots made with it still open; setting up, changing the password or making a new
//! recovery key writes the graph's own file and leaves the old one for the other graphs.

use std::path::PathBuf;

use notatnik_graph::{DeviceSettings, Dirs};
use serde::{Deserialize, Serialize};

/// The settings in force for one graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct BackupCfg {
    /// First snapshot folder; empty = `<data dir>/backups`.
    pub dir: String,
    /// Second folder; empty = none.
    pub mirror_dir: String,
    pub auto: bool,
    /// `compressed` or `none`.
    pub compression: String,
    pub encrypt: bool,
}

/// What is stored for the graph: all five once any was changed.
#[derive(Debug, Serialize, Deserialize)]
struct Stored {
    dir: String,
    mirror_dir: String,
    auto: bool,
    compression: String,
    encrypt: bool,
}

fn graph_dir(dirs: &Dirs, graph_id: &str) -> PathBuf {
    dirs.config.join("graphs").join(graph_id)
}

fn cfg_path(dirs: &Dirs, graph_id: &str) -> PathBuf {
    graph_dir(dirs, graph_id).join("backup.json")
}

/// The graph's own vault file (a new one is always written here).
pub(super) fn vault_path(dirs: &Dirs, graph_id: &str) -> PathBuf {
    graph_dir(dirs, graph_id).join("backup-vault.json")
}

/// A vault made but not yet confirmed (the recovery key was shown, the user has not said it is saved).
pub(super) fn pending_vault_path(dirs: &Dirs, graph_id: &str) -> PathBuf {
    graph_dir(dirs, graph_id).join("backup-vault.pending.json")
}

/// The vault used before backups were per graph, shared by all graphs.
fn legacy_vault_path(dirs: &Dirs) -> PathBuf {
    dirs.config.join("backup-vault.json")
}

/// The vault file to read for the graph: its own, else the old shared one, else the place its own would be.
pub(super) fn vault_to_read(dirs: &Dirs, graph_id: &str) -> PathBuf {
    let own = vault_path(dirs, graph_id);
    let legacy = legacy_vault_path(dirs);
    if !own.exists() && legacy.exists() {
        legacy
    } else {
        own
    }
}

/// The settings of the graph: its own, else the device-wide values.
pub(super) fn load(dirs: &Dirs, dev: &DeviceSettings, graph_id: &str) -> BackupCfg {
    let stored = std::fs::read(cfg_path(dirs, graph_id))
        .ok()
        .and_then(|b| serde_json::from_slice::<Stored>(&b).ok());
    match stored {
        Some(s) => BackupCfg {
            dir: s.dir,
            mirror_dir: s.mirror_dir,
            auto: s.auto,
            compression: s.compression,
            encrypt: s.encrypt,
        },
        None => BackupCfg {
            dir: dev.ui.backup_dir.clone(),
            mirror_dir: dev.ui.backup_mirror_dir.clone(),
            auto: dev.ui.backup_auto,
            compression: dev.ui.backup_compression.clone(),
            encrypt: dev.ui.backup_encrypt,
        },
    }
}

/// Writes the settings of the graph (all five), by a temporary file and a rename.
pub(super) fn save(dirs: &Dirs, graph_id: &str, cfg: &BackupCfg) -> std::io::Result<()> {
    let path = cfg_path(dirs, graph_id);
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d)?;
    }
    let stored = Stored {
        dir: cfg.dir.clone(),
        mirror_dir: cfg.mirror_dir.clone(),
        auto: cfg.auto,
        compression: cfg.compression.clone(),
        encrypt: cfg.encrypt,
    };
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(&stored)?)?;
    std::fs::rename(&tmp, path)
}

impl BackupCfg {
    /// The first snapshot folder: the setting, or the default.
    pub(super) fn primary(&self, dirs: &Dirs) -> PathBuf {
        match self.dir.trim() {
            "" => dirs.data.join("backups"),
            d => PathBuf::from(d),
        }
    }

    /// The second snapshot folder, if one is set.
    pub(super) fn mirror(&self) -> Option<PathBuf> {
        Some(self.mirror_dir.trim())
            .filter(|d| !d.is_empty())
            .map(PathBuf::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dirs() -> (tempfile::TempDir, Dirs) {
        let t = tempfile::tempdir().unwrap();
        let d = Dirs::under(&t.path().join("device"));
        (t, d)
    }

    #[test]
    fn a_graph_follows_the_device_values_until_it_has_its_own() {
        let (_t, dirs) = dirs();
        let mut dev = DeviceSettings::default();
        dev.ui.backup_dir = "/old/place".into();
        dev.ui.backup_auto = false;
        let cfg = load(&dirs, &dev, "g1");
        assert_eq!((cfg.dir.as_str(), cfg.auto), ("/old/place", false));
        // a change in one graph writes its own file; another graph and the device values are untouched
        let mut mine = cfg.clone();
        mine.dir = "/new/place".into();
        save(&dirs, "g1", &mine).unwrap();
        assert_eq!(load(&dirs, &dev, "g1").dir, "/new/place");
        assert_eq!(load(&dirs, &dev, "g2").dir, "/old/place");
        assert_eq!(dev.ui.backup_dir, "/old/place");
    }

    #[test]
    fn the_old_shared_vault_is_read_until_the_graph_has_its_own() {
        let (_t, dirs) = dirs();
        std::fs::create_dir_all(&dirs.config).unwrap();
        std::fs::write(dirs.config.join("backup-vault.json"), b"{}").unwrap();
        assert_eq!(
            vault_to_read(&dirs, "g1"),
            dirs.config.join("backup-vault.json")
        );
        // its own file wins as soon as it exists; new vaults are always written there
        std::fs::create_dir_all(vault_path(&dirs, "g1").parent().unwrap()).unwrap();
        std::fs::write(vault_path(&dirs, "g1"), b"{}").unwrap();
        assert_eq!(vault_to_read(&dirs, "g1"), vault_path(&dirs, "g1"));
        assert_eq!(
            vault_to_read(&dirs, "g2"),
            dirs.config.join("backup-vault.json")
        );
    }
}
