//! Passwords of server logins and the key of an encrypted graph on Android (SY-7, EN-10, AS1): the Kotlin plugin
//! `SecretsPlugin` (`gen/android/app/.../SecretsPlugin.kt`) keeps them encrypted with a key of the Android Keystore.
//! The app's `Secrets` trait is the only seam; nothing else knows where they are kept.

use std::sync::mpsc;
use std::time::Duration;

use notatnik_app::Secrets;
use serde::Deserialize;
use serde_json::json;
use tauri::plugin::{Builder, PluginHandle, TauriPlugin};
use tauri::{Manager, Wry};

/// How long a call to the plugin may take. The call goes to the Android UI thread and back; it is asked on a thread of
/// its own with a limit, so that a call that never comes back (a Keystore that hangs) makes a secret "not known" and
/// does not freeze whoever asked.
const LIMIT: Duration = Duration::from_secs(10);

/// The handle of the Kotlin plugin, kept as managed state until `setup` hands it to `KeystoreSecrets`.
pub struct SecretsHandle(pub PluginHandle<Wry>);

/// The plugin: registers the Kotlin class by its package and name.
pub fn plugin() -> TauriPlugin<Wry> {
    Builder::new("secrets")
        .setup(|app, api| {
            let handle = api.register_android_plugin("eu.cdfg.notatnik", "SecretsPlugin")?;
            app.manage(SecretsHandle(handle));
            Ok(())
        })
        .build()
}

#[derive(Deserialize)]
struct Value {
    value: Option<String>,
}

pub struct KeystoreSecrets(PluginHandle<Wry>);

impl KeystoreSecrets {
    pub fn new(handle: PluginHandle<Wry>) -> Self {
        Self(handle)
    }

    /// One command of the plugin, with the limit. Errors are text, as the `Secrets` trait wants them.
    fn call<T: serde::de::DeserializeOwned + Send + 'static>(
        &self,
        command: &'static str,
        payload: serde_json::Value,
    ) -> Result<T, String> {
        let (tx, rx) = mpsc::channel();
        let handle = self.0.clone();
        std::thread::spawn(move || {
            let _ = tx.send(
                handle
                    .run_mobile_plugin::<T>(command, payload)
                    .map_err(|e| e.to_string()),
            );
        });
        rx.recv_timeout(LIMIT)
            .map_err(|_| format!("the secure storage did not answer ({command})"))?
    }
}

impl Secrets for KeystoreSecrets {
    fn get(&self, key: &str) -> Option<String> {
        match self.call::<Value>("get", json!({ "key": key })) {
            Ok(v) => v.value,
            Err(e) => {
                log::warn!("a secret could not be read: {e}");
                None
            }
        }
    }

    fn set(&self, key: &str, value: &str) -> Result<(), String> {
        self.call::<serde_json::Value>("set", json!({ "key": key, "value": value }))
            .map(|_| ())
            .inspect_err(|e| log::error!("a secret could not be stored: {e}"))
    }

    fn delete(&self, key: &str) {
        if let Err(e) = self.call::<serde_json::Value>("delete", json!({ "key": key })) {
            log::warn!("a secret could not be removed: {e}");
        }
    }
}
