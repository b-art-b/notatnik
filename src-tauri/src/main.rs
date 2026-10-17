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

/// Shows the system's folder chooser and returns the folder, or `None` when the user cancels. Called from the start
/// screen and the settings to pick the folder of a graph instead of typing its path.
#[tauri::command]
async fn pick_folder(app: tauri::AppHandle) -> Option<String> {
    use tauri_plugin_dialog::DialogExt;
    // an async command runs off the main thread, where waiting for the dialog is allowed
    app.dialog()
        .file()
        .set_title("Choose the folder of your notes")
        .blocking_pick_folder()
        .map(|p| p.to_string())
}

/// Decodes `%XX` escapes (the UI sends the asset path as one `encodeURIComponent` segment).
fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        let hex = |c: u8| (c as char).to_digit(16);
        if b[i] == b'%' && i + 2 < b.len() && hex(b[i + 1]).is_some() && hex(b[i + 2]).is_some() {
            out.push((hex(b[i + 1]).unwrap() * 16 + hex(b[i + 2]).unwrap()) as u8);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn main() {
    tauri::Builder::default()
        // `asset://localhost/<path>`: the images below the graph's `assets/` folder (ST-8)
        .register_uri_scheme_protocol("asset", |ctx, req| {
            let path = percent_decode(req.uri().path().trim_start_matches('/'));
            let bytes = ctx
                .app_handle()
                .try_state::<Arc<AppState>>()
                .and_then(|s| s.read_asset(&path).ok());
            match bytes {
                Some(b) => tauri::http::Response::builder()
                    .header("Content-Type", notatnik_app::asset_content_type(&path))
                    .body(b)
                    .unwrap(),
                None => tauri::http::Response::builder()
                    .status(404)
                    .body(Vec::new())
                    .unwrap(),
            }
        })
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let handle = app.handle().clone();
            let emit: notatnik_app::Emitter = Arc::new(move |name, payload| {
                let _ = handle.emit(name, payload);
            });
            let state = Arc::new(AppState::new(AppOptions::standard(emit)));
            // reopen the last graph without asking only when the device setting says so; otherwise the start
            // screen lists the graphs, the last one selected
            if let Ok(s) = notatnik_graph::DeviceSettings::load(&notatnik_graph::Dirs::from_env()) {
                if let (true, Some(path)) = (s.ui.open_last_graph, s.last_path()) {
                    if std::path::Path::new(path).is_dir() {
                        let _ = dispatch(
                            &state,
                            "open_graph",
                            serde_json::json!({"path": path, "create": false}),
                        );
                    }
                }
            }
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            invoke_api,
            open_external,
            pick_folder
        ])
        .build(tauri::generate_context!())
        .expect("error while building Notatnik")
        .run(|app, event| {
            // a snapshot on quit (BK-3), in a thread so a slow target cannot keep the app from closing for long
            if let tauri::RunEvent::Exit = event {
                if let Some(state) = app.try_state::<Arc<AppState>>() {
                    let state = Arc::clone(&state);
                    let (done, wait) = std::sync::mpsc::channel();
                    std::thread::spawn(move || {
                        state.backup_on_quit();
                        let _ = done.send(());
                    });
                    let _ = wait.recv_timeout(std::time::Duration::from_secs(20));
                }
            }
        });
}
