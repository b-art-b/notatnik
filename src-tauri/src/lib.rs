//! Notatnik shell for the desktop and for iPad/Android (Tauri 2). All logic lives in `notatnik-app`; this crate only
//! exposes it to the webview through one command (`invoke_api`) and forwards `graph-changed` events. `main.rs` is
//! the desktop entry point, `run` below is also the mobile one (`mobile_entry_point`).

use std::sync::Arc;

#[cfg(target_os = "android")]
mod keystore;

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "ios"))]
use notatnik_app::Secrets;
use notatnik_app::{ApiError, AppOptions, AppState, dispatch, dispatch_slot};
use serde_json::Value;
use tauri::{Emitter, Manager, State};

/// `async` so that the command runs on a worker thread and not on the one that draws the window: a command that waits
/// for a web site (a page being clipped, a sync) must not freeze the app, and `clip_cancel` has to get through while
/// `clip_page` is still waiting. The state has its own locks, as the dev server (one thread per request) already needs.
#[tauri::command(async)]
fn invoke_api(
    state: State<'_, Arc<AppState>>,
    name: String,
    args: Value,
    slot: Option<String>,
) -> Result<Value, ApiError> {
    dispatch_slot(&state, slot.as_deref().unwrap_or(""), &name, args)
}

/// Opens an http(s)/mailto link in the system browser, and a `file:///` link with the system's program for that
/// file. Programs and scripts are not opened.
#[tauri::command]
fn open_external(app: tauri::AppHandle, url: String) -> Result<(), String> {
    let l = url.to_ascii_lowercase();
    if l.starts_with("file:///") {
        // `…/a.pdf#page=3`: a page or a query is not part of the path
        let path = percent_decode(
            url["file://".len()..]
                .split(['#', '?'])
                .next()
                .unwrap_or(""),
        );
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
    slot: Option<String>,
) -> Result<(), String> {
    let file = state
        .asset_file_in(slot.as_deref().unwrap_or(""), &percent_decode(&path))
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
#[cfg(desktop)]
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

/// A phone or tablet has no folder chooser for a graph: the graph is in the app's own storage (see `open_mobile_graph`).
#[cfg(mobile)]
#[tauri::command]
async fn pick_folder() -> Option<String> {
    None
}

/// The picture on the system clipboard (a screenshot from Flameshot, Spectacle ...) as a base64 PNG, or `None` when
/// the clipboard holds no picture. The webview does not hand such a picture to a `paste` event on every system, so
/// the editor asks here when a paste brings no text and no file (ST-8).
#[cfg(desktop)]
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

/// Phones and tablets paste a picture through the webview's own `paste` event; there is no clipboard to ask here.
#[cfg(mobile)]
#[tauri::command]
async fn clipboard_image() -> Option<String> {
    None
}

/// Asks where to save a file and writes it (DR-22: a drawing as PNG or SVG). `data` is the file as base64; returns the
/// path written, or `None` when the person cancels.
#[cfg(desktop)]
#[tauri::command]
async fn save_export(
    app: tauri::AppHandle,
    name: String,
    data: String,
) -> Result<Option<String>, String> {
    use base64::Engine as _;
    use tauri_plugin_dialog::DialogExt;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data)
        .map_err(|e| e.to_string())?;
    // a name from the page is only a suggestion: nothing but its last part is used
    let name = name
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or("drawing")
        .to_string();
    let Some(path) = app
        .dialog()
        .file()
        .set_title("Save the drawing")
        .set_file_name(name)
        .blocking_save_file()
    else {
        return Ok(None);
    };
    let path = path.into_path().map_err(|e| e.to_string())?;
    std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
    Ok(Some(path.to_string_lossy().into_owned()))
}

/// A phone or tablet has no save dialog: the UI copies the picture instead.
#[cfg(mobile)]
#[tauri::command]
async fn save_export(_name: String, _data: String) -> Result<Option<String>, String> {
    Ok(None)
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

/// Passwords of server logins go in the system's keyring (the Secret Service: GNOME Keyring, KWallet; the Keychain on
/// macOS and iOS), never in a file.
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "ios"))]
struct KeyringSecrets;

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "ios"))]
impl KeyringSecrets {
    fn entry(key: &str) -> Result<keyring::Entry, keyring::Error> {
        keyring::Entry::new("notatnik", key)
    }
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "ios"))]
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

/// Sets `window.__NOTATNIK_MOBILE__` before the page runs (`ui/src/lib/platform.ts`). A plugin without commands: it
/// needs no permission.
#[cfg(mobile)]
fn mobile_flag() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    tauri::plugin::Builder::new("device")
        .js_init_script("window.__NOTATNIK_MOBILE__ = true;")
        .build()
}

/// The device folders (settings, caches, keys of the backup vault): the XDG folders on a desktop; on iOS and Android
/// the app's own data folder, the only place the system lets an app write to (PL-5).
fn device_dirs(app: &tauri::App) -> notatnik_graph::Dirs {
    #[cfg(mobile)]
    if let Ok(dir) = app.path().app_data_dir() {
        return notatnik_graph::Dirs::under(&dir);
    }
    let _ = app;
    notatnik_graph::Dirs::from_env()
}

/// Phones and tablets have no start screen with a folder chooser: the graph lives in the app's own storage (the
/// system gives no other place without a security-scoped bookmark) and is kept in step with a server by the built-in
/// sync. The last graph opens again; the first start makes `notes` there.
#[cfg(mobile)]
fn open_mobile_graph(app: &tauri::App, state: &Arc<AppState>, dirs: &notatnik_graph::Dirs) {
    let last = notatnik_graph::DeviceSettings::load(dirs)
        .ok()
        .and_then(|s| s.last_path().map(str::to_string))
        .filter(|p| std::path::Path::new(p).is_dir());
    let (path, create) = match last {
        Some(p) => (p, false),
        None => match app.path().app_data_dir() {
            Ok(d) => (d.join("notes").to_string_lossy().into_owned(), true),
            Err(e) => {
                log::error!("no data folder for the first graph: {e}");
                return;
            }
        },
    };
    if let Err(e) = dispatch(
        state,
        "open_graph",
        serde_json::json!({"path": path, "create": create}),
    ) {
        log::error!("the graph ({path}) could not be opened: {e}");
    }
}

/// Starts the app (the desktop `main` calls this; on iOS and Android the platform's launcher does).
// the mobile entry point macro declares a `#[no_mangle]` function, which the crate-wide `unsafe_code = "deny"` flags
#[cfg_attr(mobile, tauri::mobile_entry_point)]
#[cfg_attr(mobile, allow(unsafe_code))]
pub fn run() {
    notatnik_log::init("info");
    log::info!(
        "Notatnik {} starting (data in {})",
        env!("CARGO_PKG_VERSION"),
        notatnik_graph::Dirs::from_env().data.display()
    );
    let builder = tauri::Builder::default();
    // the passwords of Android: a Kotlin plugin around the Android Keystore (AS1)
    #[cfg(target_os = "android")]
    let builder = builder.plugin(keystore::plugin());
    // a phone or tablet: the UI leaves out what only a computer has (a folder chooser, paths of a computer)
    #[cfg(mobile)]
    let builder = builder.plugin(mobile_flag());
    builder
        // `asset://localhost/<path>`: the images below the graph's `assets/` folder (ST-8)
        .register_uri_scheme_protocol("asset", |ctx, req| {
            let path = percent_decode(req.uri().path().trim_start_matches('/'));
            // `?slot=1`: the picture is in the graph of that slot (MG-1)
            let slot = req
                .uri()
                .query()
                .and_then(|q| q.split('&').find_map(|kv| kv.strip_prefix("slot=")))
                .unwrap_or("")
                .to_string();
            let bytes = ctx
                .app_handle()
                .try_state::<Arc<AppState>>()
                .and_then(|s| s.read_asset_in(&slot, &path).ok());
            match bytes {
                Some(b) => tauri::http::Response::builder()
                    .header("Content-Type", notatnik_app::asset_content_type(&path))
                    // the PDF viewer reads a PDF with `fetch` (PA-1), which a page of another origin may do only
                    // with this header; an `<img>` never needed it
                    .header("Access-Control-Allow-Origin", "*")
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
            let dirs = device_dirs(app);
            let options = AppOptions {
                dirs: dirs.clone(),
                ..AppOptions::standard(emit)
            };
            let state = AppState::new(options);
            #[cfg(any(target_os = "linux", target_os = "macos", target_os = "ios"))]
            let state = state.with_secrets(Arc::new(KeyringSecrets));
            #[cfg(target_os = "android")]
            let state = state.with_secrets(Arc::new(keystore::KeystoreSecrets::new(
                app.state::<keystore::SecretsHandle>().0.clone(),
            )));
            // a computer can read its own files for the viewers (LV-1); a phone or tablet cannot read a path of one
            #[cfg(desktop)]
            let state = state.with_local_files();
            let state = Arc::new(state);
            // reopen the last graph without asking only when the device setting says so; otherwise the start
            // screen lists the graphs, the last one selected
            if let Ok(s) = notatnik_graph::DeviceSettings::load(&dirs) {
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
            #[cfg(mobile)]
            open_mobile_graph(app, &state, &dirs);
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            invoke_api,
            open_external,
            pick_folder,
            pick_file,
            open_asset,
            clipboard_image,
            save_export
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
