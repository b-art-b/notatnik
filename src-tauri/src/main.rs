//! Notatnik desktop shell. All logic lives in `notatnik-app`; this file only exposes it to the webview
//! through one command (`invoke_api`) and forwards `graph-changed` events.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Arc;

use notatnik_app::{ApiError, AppOptions, AppState, Secrets, dispatch};
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

/// Opens an http(s)/mailto link in the system browser, and a `file:///` link with the system's program for that
/// file. Programs and scripts are not opened.
#[tauri::command]
fn open_external(app: tauri::AppHandle, url: String) -> Result<(), String> {
    let l = url.to_ascii_lowercase();
    if l.starts_with("file:///") {
        let path = percent_decode(&url["file://".len()..]);
        return open_local(&app, std::path::Path::new(&path));
    }
    if !(l.starts_with("http://") || l.starts_with("https://") || l.starts_with("mailto:")) {
        log::warn!("a link was not opened, only http, https, mailto and file links are");
        return Err("only http, https, mailto and file links can be opened".into());
    }
    tauri_plugin_opener::OpenerExt::opener(&app)
        .open_url(url, None::<&str>)
        .map_err(|e| e.to_string())
}

/// Opens a file of this computer with the program the system uses for it.
fn open_local(app: &tauri::AppHandle, path: &std::path::Path) -> Result<(), String> {
    let name = path
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    if notatnik_graph::is_risky_file(&name) {
        log::warn!("{name} was not opened: programs and scripts are not opened from a note");
        return Err("programs and scripts are not opened from a note".into());
    }
    if !path.exists() {
        return Err(format!(
            "{} does not exist on this computer",
            path.display()
        ));
    }
    tauri_plugin_opener::OpenerExt::opener(app)
        .open_path(path.to_string_lossy(), None::<&str>)
        .map_err(|e| e.to_string())
}

/// Opens a file that was copied into the graph (`../assets/2026/10/a.pdf` as a link writes it).
#[tauri::command]
fn open_asset(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    path: String,
) -> Result<(), String> {
    let file = state
        .asset_file(&percent_decode(&path))
        .map_err(|e| e.message)?;
    open_local(&app, &file)
}

/// Shows the system's file chooser and returns the file, or `None` when the user cancels (for `/file`).
#[tauri::command]
async fn pick_file(app: tauri::AppHandle) -> Option<String> {
    use tauri_plugin_dialog::DialogExt;
    app.dialog()
        .file()
        .set_title("Choose a file to link")
        .blocking_pick_file()
        .map(|p| p.to_string())
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

/// The picture on the system clipboard (a screenshot from Flameshot, Spectacle ...) as a base64 PNG, or `None` when
/// the clipboard holds no picture. The webview does not hand such a picture to a `paste` event on every system, so
/// the editor asks here when a paste brings no text and no file (ST-8).
#[tauri::command]
async fn clipboard_image() -> Option<String> {
    use base64::Engine as _;
    let img = arboard::Clipboard::new().ok()?.get_image().ok()?;
    let (w, h) = (
        u32::try_from(img.width).ok()?,
        u32::try_from(img.height).ok()?,
    );
    let mut out = Vec::new();
    let mut enc = png::Encoder::new(&mut out, w, h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header().ok()?.write_image_data(&img.bytes).ok()?;
    Some(base64::engine::general_purpose::STANDARD.encode(out))
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

/// Passwords of server logins go in the system's keyring (the Secret Service: GNOME Keyring, KWallet), never in a file.
struct KeyringSecrets;

impl KeyringSecrets {
    fn entry(key: &str) -> Result<keyring::Entry, keyring::Error> {
        keyring::Entry::new("notatnik", key)
    }
}

impl Secrets for KeyringSecrets {
    fn get(&self, key: &str) -> Option<String> {
        Self::entry(key).ok()?.get_password().ok()
    }
    fn set(&self, key: &str, value: &str) -> Result<(), String> {
        Self::entry(key)
            .and_then(|e| e.set_password(value))
            .map_err(|e| {
                log::error!("the keyring did not take the password: {e}");
                format!("{e} (is a keyring such as GNOME Keyring running?)")
            })
    }
    fn delete(&self, key: &str) {
        if let Ok(e) = Self::entry(key) {
            let _ = e.delete_credential();
        }
    }
}

fn main() {
    notatnik_log::init("info");
    log::info!(
        "Notatnik {} starting (data in {})",
        env!("CARGO_PKG_VERSION"),
        notatnik_graph::Dirs::from_env().data.display()
    );
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
            let state = Arc::new(
                AppState::new(AppOptions::standard(emit)).with_secrets(Arc::new(KeyringSecrets)),
            );
            // reopen the last graph without asking only when the device setting says so; otherwise the start
            // screen lists the graphs, the last one selected
            if let Ok(s) = notatnik_graph::DeviceSettings::load(&notatnik_graph::Dirs::from_env()) {
                if let (true, Some(path)) = (s.ui.open_last_graph, s.last_path()) {
                    if std::path::Path::new(path).is_dir() {
                        if let Err(e) = dispatch(
                            &state,
                            "open_graph",
                            serde_json::json!({"path": path, "create": false}),
                        ) {
                            log::error!("the last graph ({path}) could not be reopened: {e}");
                        }
                    }
                }
            }
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            invoke_api,
            open_external,
            pick_folder,
            pick_file,
            open_asset,
            clipboard_image
        ])
        .build(tauri::generate_context!())
        .expect("error while building Notatnik")
        .run(|app, event| {
            // a last sync (SY-6) and a snapshot on quit (BK-3), in a thread so a slow server or target cannot keep
            // the app from closing for long
            if let tauri::RunEvent::Exit = event {
                if let Some(state) = app.try_state::<Arc<AppState>>() {
                    log::info!("quitting: a last sync and a backup, if they are due");
                    let started = std::time::Instant::now();
                    let state = Arc::clone(&state);
                    let (done, wait) = std::sync::mpsc::channel();
                    std::thread::spawn(move || {
                        state.sync_on_quit();
                        state.backup_on_quit();
                        let _ = done.send(());
                    });
                    match wait.recv_timeout(std::time::Duration::from_secs(30)) {
                        Ok(()) => log::info!("bye ({} ms)", started.elapsed().as_millis()),
                        Err(_) => log::warn!(
                            "the last sync or backup took more than 30 s; quitting without waiting"
                        ),
                    }
                }
            }
        });
}
