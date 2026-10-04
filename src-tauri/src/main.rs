//! Notatnik desktop shell. All logic lives in `notatnik-app`; this file only exposes it to the webview
//! through one command (`invoke_api`) and forwards `graph-changed` events.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Arc;

use notatnik_app::{ApiError, AppOptions, AppState, dispatch};
use serde_json::Value;
use tauri::{Emitter, Manager, State};

#[tauri::command]
fn invoke_api(
    state: State<'_, Arc<AppState>>,
    name: String,
    args: Value,
) -> Result<Value, ApiError> {
    dispatch(&state, &name, args)
}

/// Opens an http(s)/mailto link in the system browser.
#[tauri::command]
fn open_external(app: tauri::AppHandle, url: String) -> Result<(), String> {
    let l = url.to_ascii_lowercase();
    if !(l.starts_with("http://") || l.starts_with("https://") || l.starts_with("mailto:")) {
        return Err("only http, https and mailto links can be opened".into());
    }
    tauri_plugin_opener::OpenerExt::opener(&app)
        .open_url(url, None::<&str>)
        .map_err(|e| e.to_string())
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let handle = app.handle().clone();
            let emit: notatnik_app::Emitter = Arc::new(move |name, payload| {
                let _ = handle.emit(name, payload);
            });
            let state = Arc::new(AppState::new(AppOptions::standard(emit)));
            // reopen the last graph if the device settings know one
            if let Ok(s) = notatnik_graph::DeviceSettings::load(&notatnik_graph::Dirs::from_env()) {
                if let Some(g) = s.graphs.last() {
                    let _ = dispatch(
                        &state,
                        "open_graph",
                        serde_json::json!({"path": g.path, "create": false}),
                    );
                }
            }
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![invoke_api, open_external])
        .run(tauri::generate_context!())
        .expect("error while running Notatnik");
}
