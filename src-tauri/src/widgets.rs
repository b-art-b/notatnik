//! The home-screen widgets of Android (AN-5..AN-7, `docs/phases/android-widgets.md`): the Kotlin plugin `WidgetsPlugin`
//! (`gen/android/app/.../WidgetsPlugin.kt`) behind the app's `Widgets` trait. The widgets themselves are Kotlin; this
//! only tells them where the files are, asks them to redraw, and passes on the link the app was opened with.

use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;

use notatnik_app::Widgets;
use serde::Deserialize;
use serde_json::json;
use tauri::plugin::{Builder, PluginHandle, TauriPlugin};
use tauri::{Manager, Wry};

/// How long a call to the plugin may take; a call that never comes back must not freeze the command that asked.
const LIMIT: Duration = Duration::from_secs(5);

/// The handle of the Kotlin plugin, kept as managed state until `setup` hands it to [`AndroidWidgets`].
pub struct WidgetsHandle(pub PluginHandle<Wry>);

/// The plugin: registers the Kotlin class by its package and name.
pub fn plugin() -> TauriPlugin<Wry> {
    Builder::new("widgets")
        .setup(|app, api| {
            let handle = api.register_android_plugin("eu.cdfg.notatnik", "WidgetsPlugin")?;
            app.manage(WidgetsHandle(handle));
            Ok(())
        })
        .build()
}

#[derive(Deserialize)]
struct Link {
    value: Option<String>,
}

pub struct AndroidWidgets(PluginHandle<Wry>);

impl AndroidWidgets {
    pub fn new(handle: PluginHandle<Wry>) -> Self {
        Self(handle)
    }

    fn call<T: serde::de::DeserializeOwned + Send + 'static>(
        &self,
        command: &'static str,
        payload: serde_json::Value,
    ) -> Option<T> {
        let (tx, rx) = mpsc::channel();
        let handle = self.0.clone();
        std::thread::spawn(move || {
            let _ = tx.send(handle.run_mobile_plugin::<T>(command, payload));
        });
        match rx.recv_timeout(LIMIT) {
            Ok(Ok(v)) => Some(v),
            Ok(Err(e)) => {
                log::warn!("the widgets plugin ({command}): {e}");
                None
            }
            Err(_) => {
                log::warn!("the widgets plugin did not answer ({command})");
                None
            }
        }
    }
}

impl Widgets for AndroidWidgets {
    fn directory(&self, dir: &Path) {
        let _ =
            self.call::<serde_json::Value>("directory", json!({ "dir": dir.to_string_lossy() }));
    }

    fn refresh(&self) {
        let _ = self.call::<serde_json::Value>("refresh", json!({}));
    }

    fn take_link(&self) -> Option<String> {
        self.call::<Link>("takeLink", json!({}))
            .and_then(|l| l.value)
            .filter(|l| !l.is_empty())
    }
}
