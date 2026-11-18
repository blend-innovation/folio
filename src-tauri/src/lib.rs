//! Folio's native shell.
//!
//! The shell is deliberately thin: it owns the window, menus, file dialogs,
//! recent documents, live file watching, and the theme folder. All Markdown
//! interpretation lives in `folio-core`.

use base64::Engine as _;
use folio_core::theme::ThemeCatalog;
use folio_core::theme::ThemeSet;
use folio_core::{render, DocumentMeta, RenderOptions};
use notify::{RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::menu::{AboutMetadataBuilder, MenuBuilder, MenuItem, SubmenuBuilder};
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder, Wry};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;
use tauri_plugin_window_state::StateFlags;

const MAX_DOCUMENT_BYTES: u64 = 16 * 1024 * 1024;
const MAX_IMAGE_BYTES: u64 = 24 * 1024 * 1024;
const MAX_PREVIEW_BYTES: usize = 8 * 1024 * 1024;
const SAMPLE_MARKDOWN: &str = include_str!("../../docs/sample.md");
const SUPPORTED_EXTENSIONS: &[&str] = &[
    "md", "markdown", "mdown", "mkd", "mkdn", "mdx", "txt", "text",
];

/// macOS may deliver an `openURLs` AppleEvent before Tauri has run `.setup`.
/// Keep that path outside managed state so a launch-through-Finder open cannot
/// panic before the app is initialized.
#[cfg(target_os = "macos")]
static PENDING_OPEN: Mutex<Option<PathBuf>> = Mutex::new(None);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentFile {
    pub path: String,
    pub name: String,
    pub directory: String,
    pub opened_at: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentPayload {
    pub path: String,
    pub name: String,
    pub html: String,
    pub meta: DocumentMeta,
    pub size: u64,
    pub modified_ms: u64,
    pub sample: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DefaultAppStatus {
    pub available: bool,
    pub is_default: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Bootstrap {
    pub themes: ThemeCatalog,
    pub recents: Vec<RecentFile>,
    pub version: String,
    pub initial_path: Option<String>,
    pub themes_dir: String,
    pub platform: String,
}

pub struct AppState {
    initial_path: Mutex<Option<PathBuf>>,
    open_lock: Mutex<()>,
    booted: AtomicBool,
    watcher: Mutex<Option<notify::RecommendedWatcher>>,
    recents: Mutex<Vec<RecentFile>>,
    preview_html: Mutex<Option<String>>,
    themes: ThemeCatalog,
    themes_dir: PathBuf,
}

impl AppState {
    fn new(themes: ThemeCatalog, recents: Vec<RecentFile>, themes_dir: PathBuf) -> Self {
        Self {
            initial_path: Mutex::new(None),
            open_lock: Mutex::new(()),
            booted: AtomicBool::new(false),
            watcher: Mutex::new(None),
            recents: Mutex::new(recents),
            preview_html: Mutex::new(None),
            themes,
            themes_dir,
        }
    }
}

/* --------------------------------------------------------------------------
Commands
-------------------------------------------------------------------------- */

#[tauri::command]
fn bootstrap(state: State<'_, AppState>) -> Bootstrap {
    let (initial_path, recents) = {
        let _guard = state.open_lock.lock().unwrap();
        let initial = state.initial_path.lock().unwrap().take();
        state.booted.store(true, Ordering::SeqCst);
        (initial, state.recents.lock().unwrap().clone())
    };
    Bootstrap {
        themes: state.themes.clone(),
        recents,
        version: env!("CARGO_PKG_VERSION").to_string(),
        initial_path: initial_path.map(|path| path.to_string_lossy().into_owned()),
        themes_dir: state.themes_dir.to_string_lossy().into_owned(),
        platform: std::env::consts::OS.to_string(),
    }
}

/// Explicit native drag fallback. This bypasses Tauri's injected drag-region
/// handler, which depends on a separate capability and only starts a drag when
/// a `data-tauri-drag-region` element is hit.
#[tauri::command]
fn start_window_drag(app: AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "No main window".to_string())?;
    window.start_dragging().map_err(|error| error.to_string())
}

/// HTML previews are deliberately kept out of the Markdown document itself:
/// the source block is shown as text, and this command opens a separate native
/// window whose content page isolates the preview in a sandboxed iframe.
#[tauri::command]
async fn open_html_preview(
    app: AppHandle,
    state: State<'_, AppState>,
    html: String,
    title: Option<String>,
) -> Result<(), String> {
    if html.len() > MAX_PREVIEW_BYTES {
        return Err("HTML preview is too large (limit: 8 MB).".into());
    }

    let title = preview_title(title.as_deref());
    *state.preview_html.lock().unwrap() = Some(html);

    if let Some(window) = app.get_webview_window("html-preview") {
        let _ = window.set_title(&title);
        let _ = window.eval("window.location.reload()");
        let _ = window.show();
        let _ = window.set_focus();
        return Ok(());
    }

    WebviewWindowBuilder::new(&app, "html-preview", WebviewUrl::App("preview.html".into()))
        .title(&title)
        .inner_size(1024.0, 768.0)
        .min_inner_size(520.0, 360.0)
        .resizable(true)
        .build()
        .map(|_| ())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn html_preview_source(state: State<'_, AppState>) -> Option<String> {
    state.preview_html.lock().unwrap().clone()
}

fn preview_title(title: Option<&str>) -> String {
    let first = title.unwrap_or_default().lines().next().unwrap_or_default();
    let cleaned = first
        .chars()
        .filter(|ch| !ch.is_control())
        .take(80)
        .collect::<String>();
    let cleaned = cleaned.trim();
    if cleaned.is_empty() {
        "HTML Preview — Folio".into()
    } else {
        format!("{cleaned} — Folio")
    }
}

#[tauri::command]
fn show_window(app: AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

#[tauri::command]
fn set_title(app: AppHandle, title: String) -> Result<(), String> {
    let clean: String = title
        .chars()
        .filter(|c| !c.is_control())
        .take(180)
        .collect();
    if let Some(window) = app.get_webview_window("main") {
        window.set_title(&clean).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn render_document(
    app: AppHandle,
    path: String,
    anchor: Option<String>,
) -> Result<DocumentPayload, String> {
    let _ = anchor;
    let path = path_from_input(&path)?;
    let payload = load_document(&path)?;
    state_watch(&app, &path);
    record_recent(&app, &path);
    Ok(payload)
}

#[tauri::command]
async fn open_file_dialog(app: AppHandle) -> Result<Option<DocumentPayload>, String> {
    let selected = app
        .dialog()
        .file()
        .add_filter(
            "Markdown",
            &["md", "markdown", "mdown", "mkd", "mkdn", "mdx"],
        )
        .add_filter("Text", &["txt", "text"])
        .blocking_pick_file();
    let Some(file) = selected else {
        return Ok(None);
    };
    let path = file.into_path().map_err(|e| e.to_string())?;
    let payload = load_document(&path)?;
    state_watch(&app, &path);
    record_recent(&app, &path);
    Ok(Some(payload))
}

#[tauri::command]
fn open_sample(app: AppHandle) -> Result<DocumentPayload, String> {
    // A sample is not watched; stop watching the previous document.
    *app.state::<AppState>().watcher.lock().unwrap() = None;
    let rendered = render(SAMPLE_MARKDOWN, &RenderOptions::default());
    Ok(DocumentPayload {
        path: String::new(),
        name: "sample.md".to_string(),
        html: rendered.html,
        meta: rendered.meta,
        size: SAMPLE_MARKDOWN.len() as u64,
        modified_ms: 0,
        sample: true,
    })
}

#[tauri::command]
fn read_image(path: String) -> Result<String, String> {
    let path = PathBuf::from(&path);
    let metadata = std::fs::metadata(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    if !metadata.is_file() {
        return Err("Not an image file.".into());
    }
    if metadata.len() > MAX_IMAGE_BYTES {
        return Err("That image is too large to preview.".into());
    }
    let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mime = image_mime(&bytes)
        .ok_or_else(|| "Folio previews PNG, JPEG, GIF, WebP, and AVIF images.".to_string())?;
    let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
    Ok(format!("data:{mime};base64,{encoded}"))
}

#[tauri::command]
fn open_external(app: AppHandle, url: String) -> Result<(), String> {
    let parsed = tauri::Url::parse(url.trim()).map_err(|e| e.to_string())?;
    match parsed.scheme() {
        "http" | "https" | "ftp" | "mailto" | "tel" => {}
        _ => return Err("Folio only opens web, email, and phone links.".into()),
    }
    app.opener()
        .open_url(parsed.to_string(), None::<&str>)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn open_path(app: AppHandle, path: String) -> Result<(), String> {
    let path = PathBuf::from(path);
    if !path.exists() {
        return Err("That file no longer exists.".into());
    }
    app.opener()
        .open_path(path.to_string_lossy(), None::<&str>)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn reveal_themes(app: AppHandle) -> Result<(), String> {
    let dir = app.state::<AppState>().themes_dir.clone();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    app.opener()
        .open_path(dir.to_string_lossy(), None::<&str>)
        .map_err(|e| e.to_string())
}

/// Whether `.md` / `.markdown` currently resolve to this app. `available` is
/// false outside a macOS app bundle (for example `cargo run`), so the UI never
/// offers a default-app change from a development binary.
#[tauri::command]
fn markdown_default_status(app: AppHandle) -> DefaultAppStatus {
    #[cfg(target_os = "macos")]
    {
        macos_default::status(&app.config().identifier)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = app;
        DefaultAppStatus {
            available: false,
            is_default: false,
        }
    }
}

/// Ask LaunchServices to make Folio the handler for `.md` and, best effort,
/// `.markdown`. Only meaningful from the bundled app.
#[tauri::command]
fn set_markdown_default(app: AppHandle) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        macos_default::set(&app.config().identifier)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = app;
        Err("Setting the default Markdown app is only available on macOS.".into())
    }
}

/* --------------------------------------------------------------------------
Documents
-------------------------------------------------------------------------- */

fn supported(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
        .is_some_and(|ext| SUPPORTED_EXTENSIONS.contains(&ext.as_str()))
}

fn load_document(path: &Path) -> Result<DocumentPayload, String> {
    let path = std::fs::canonicalize(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if !path.is_file() {
        return Err("That is not a file.".into());
    }
    if !supported(&path) {
        return Err("Folio reads Markdown and plain text files.".into());
    }
    let metadata = std::fs::metadata(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    if metadata.len() > MAX_DOCUMENT_BYTES {
        return Err("That file is larger than 16 MB. Folio is a reader, not a database.".into());
    }
    let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let source = String::from_utf8_lossy(&bytes);
    let rendered = render(&source, &RenderOptions::for_file(&path));
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("Untitled")
        .to_string();
    let modified_ms = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |duration| duration.as_millis() as u64);
    Ok(DocumentPayload {
        path: path.to_string_lossy().into_owned(),
        name,
        html: rendered.html,
        meta: rendered.meta,
        size: metadata.len(),
        modified_ms,
        sample: false,
    })
}

fn state_watch(app: &AppHandle, path: &Path) {
    match watch_document(app, path) {
        Ok(()) => {}
        Err(error) => {
            let _ = app.emit(
                "folio:document-error",
                serde_json::json!({ "message": format!("Live reload is unavailable: {error}") }),
            );
        }
    }
}

fn watch_document(app: &AppHandle, path: &Path) -> Result<(), String> {
    let target = path.to_path_buf();
    let directory = target
        .parent()
        .ok_or_else(|| "The file has no parent directory.".to_string())?
        .to_path_buf();

    let (tx, rx) = std::sync::mpsc::channel();
    let mut watcher = notify::recommended_watcher(move |result: notify::Result<notify::Event>| {
        if let Ok(event) = result {
            let _ = tx.send(event);
        }
    })
    .map_err(|e| e.to_string())?;
    watcher
        .watch(&directory, RecursiveMode::NonRecursive)
        .map_err(|e| e.to_string())?;

    app.state::<AppState>()
        .watcher
        .lock()
        .unwrap()
        .replace(watcher);

    let app = app.clone();
    std::thread::spawn(move || {
        while let Ok(first) = rx.recv() {
            let mut relevant = event_touches(&first, &target);
            let deadline = Instant::now() + Duration::from_millis(160);
            while let Some(remaining) = deadline.checked_duration_since(Instant::now()) {
                match rx.recv_timeout(remaining) {
                    Ok(event) => relevant |= event_touches(&event, &target),
                    Err(_) => break,
                }
            }
            if !relevant {
                continue;
            }
            // Writers finish slightly after the first event; a short pause
            // avoids rendering a half-written file.
            std::thread::sleep(Duration::from_millis(60));
            let result = load_document(&target).or_else(|_| {
                std::thread::sleep(Duration::from_millis(90));
                load_document(&target)
            });
            match result {
                Ok(payload) => {
                    let _ = app.emit("folio:document-update", payload);
                }
                Err(error) => {
                    let _ = app.emit(
                        "folio:document-error",
                        serde_json::json!({
                            "message": error,
                            "path": target.to_string_lossy(),
                        }),
                    );
                }
            }
        }
    });

    Ok(())
}

fn event_touches(event: &notify::Event, target: &Path) -> bool {
    event.paths.iter().any(|path| {
        path == target
            || path
                .file_name()
                .is_some_and(|name| Some(name) == target.file_name())
    })
}

fn path_from_input(input: &str) -> Result<PathBuf, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("No document was selected.".into());
    }
    if trimmed.starts_with("file:") {
        if !valid_percent_escapes(trimmed) {
            return Err("Invalid file URL.".into());
        }
        let url = tauri::Url::parse(trimmed).map_err(|e| e.to_string())?;
        return url
            .to_file_path()
            .map_err(|_| "Invalid file URL.".to_string());
    }
    Ok(PathBuf::from(trimmed))
}

fn valid_percent_escapes(value: &str) -> bool {
    let bytes = value.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            if index + 2 >= bytes.len()
                || !bytes[index + 1].is_ascii_hexdigit()
                || !bytes[index + 2].is_ascii_hexdigit()
            {
                return false;
            }
            index += 3;
        } else {
            index += 1;
        }
    }
    true
}

fn image_mime(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Some("image/png");
    }
    if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        return Some("image/jpeg");
    }
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return Some("image/gif");
    }
    if bytes.len() >= 12
        && bytes.starts_with(b"RIFF")
        && bytes.get(8..12) == Some(b"WEBP".as_slice())
    {
        return Some("image/webp");
    }
    if bytes.len() >= 12 && bytes.get(4..8) == Some(b"ftyp".as_slice()) {
        let brand = bytes.get(8..12);
        if brand == Some(b"avif".as_slice()) || brand == Some(b"avis".as_slice()) {
            return Some("image/avif");
        }
    }
    None
}

/* --------------------------------------------------------------------------
Recents
-------------------------------------------------------------------------- */

fn config_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path().app_config_dir().map_err(|e| e.to_string())
}

fn recents_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(config_dir(app)?.join("recents.json"))
}

fn load_recents(app: &AppHandle) -> Vec<RecentFile> {
    let Ok(path) = recents_path(app) else {
        return Vec::new();
    };
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut recents: Vec<RecentFile> = serde_json::from_str(&text).unwrap_or_default();
    recents.retain(|recent| Path::new(&recent.path).is_file());
    recents.truncate(10);
    recents
}

fn record_recent(app: &AppHandle, path: &Path) {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("Untitled")
        .to_string();
    let directory = path
        .parent()
        .map(|parent| parent.to_string_lossy().into_owned())
        .unwrap_or_default();
    let recent = RecentFile {
        path: path.to_string_lossy().into_owned(),
        name,
        directory,
        opened_at: now_ms(),
    };

    let state = app.state::<AppState>();
    let list = {
        let mut recents = state.recents.lock().unwrap();
        recents.retain(|item| item.path != recent.path);
        recents.insert(0, recent);
        recents.truncate(10);
        recents.clone()
    };
    if let Ok(path) = recents_path(app) {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(text) = serde_json::to_vec_pretty(&list) {
            let temp = path.with_extension("json.tmp");
            if std::fs::write(&temp, text).is_ok() {
                let _ = std::fs::rename(&temp, &path);
            }
        }
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis() as u64)
}

/* --------------------------------------------------------------------------
Menu
-------------------------------------------------------------------------- */

fn item(
    app: &AppHandle,
    id: &str,
    label: &str,
    accelerator: Option<&str>,
) -> tauri::Result<MenuItem<Wry>> {
    MenuItem::with_id(app, id, label, true, accelerator)
}

fn build_menu(app: &AppHandle) -> tauri::Result<tauri::menu::Menu<Wry>> {
    let about = AboutMetadataBuilder::new()
        .name(Some("Folio"))
        .version(Some(env!("CARGO_PKG_VERSION")))
        .short_version(Some(env!("CARGO_PKG_VERSION")))
        .copyright(Some("© 2026 Folio contributors"))
        .credits(Some("A quiet reader for Markdown."))
        .build();

    let app_menu = SubmenuBuilder::new(app, "Folio")
        .about(Some(about))
        .separator()
        .item(&item(app, "settings", "Settings…", Some("CmdOrCtrl+,"))?)
        .separator()
        .services()
        .separator()
        .hide()
        .hide_others()
        .show_all()
        .separator()
        .quit()
        .build()?;

    let file_menu = SubmenuBuilder::new(app, "File")
        .item(&item(app, "open", "Open…", Some("CmdOrCtrl+O"))?)
        .item(&item(
            app,
            "open-sample",
            "Open Sample",
            Some("CmdOrCtrl+Shift+O"),
        )?)
        .separator()
        .item(&item(app, "reload", "Reload", Some("CmdOrCtrl+R"))?)
        .separator()
        .close_window()
        .build()?;

    let edit_menu = SubmenuBuilder::new(app, "Edit")
        .undo()
        .redo()
        .separator()
        .cut()
        .copy()
        .paste()
        .select_all()
        .build()?;

    let view_menu = SubmenuBuilder::new(app, "View")
        .item(&item(
            app,
            "toggle-outline",
            "Toggle Contents",
            Some("CmdOrCtrl+B"),
        )?)
        .item(&item(app, "find", "Find…", Some("CmdOrCtrl+F"))?)
        .separator()
        .item(&item(
            app,
            "text-larger",
            "Larger Text",
            Some("CmdOrCtrl+="),
        )?)
        .item(&item(
            app,
            "text-smaller",
            "Smaller Text",
            Some("CmdOrCtrl+-"),
        )?)
        .item(&item(
            app,
            "text-reset",
            "Actual Text Size",
            Some("CmdOrCtrl+0"),
        )?)
        .build()?;

    let window_menu = SubmenuBuilder::new(app, "Window")
        .minimize()
        .fullscreen()
        .build()?;

    MenuBuilder::new(app)
        .item(&app_menu)
        .item(&file_menu)
        .item(&edit_menu)
        .item(&view_menu)
        .item(&window_menu)
        .build()
}

fn handle_menu(app: &AppHandle, event: tauri::menu::MenuEvent) {
    let action = match event.id().as_ref() {
        "settings" => "theme",
        "open" => "open",
        "open-sample" => "open-sample",
        "reload" => "reload",
        "toggle-outline" => "toggle-outline",
        "find" => "find",
        "text-larger" => "text-larger",
        "text-smaller" => "text-smaller",
        "text-reset" => "text-reset",
        _ => return,
    };
    focus_main_window(app);
    let _ = app.emit("folio:menu", serde_json::json!({ "action": action }));
}

/* --------------------------------------------------------------------------
Host file-open events
-------------------------------------------------------------------------- */

#[cfg(target_os = "macos")]
fn register_open(app: &AppHandle, path: PathBuf) {
    let Some(state) = app.try_state::<AppState>() else {
        *PENDING_OPEN.lock().unwrap() = Some(path);
        return;
    };
    let emit = {
        let _guard = state.open_lock.lock().unwrap();
        if state.booted.load(Ordering::SeqCst) {
            true
        } else {
            *state.initial_path.lock().unwrap() = Some(path.clone());
            false
        }
    };
    if emit {
        focus_main_window(app);
        let _ = app.emit(
            "folio:open-document",
            serde_json::json!({ "path": path.to_string_lossy() }),
        );
    }
}

fn focus_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/* --------------------------------------------------------------------------
App
-------------------------------------------------------------------------- */

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let initial_path = std::env::args()
        .skip(1)
        .filter(|arg| !arg.starts_with('-'))
        .map(PathBuf::from)
        .find(|path| path.is_file());

    let builder = tauri::Builder::default()
        .plugin(
            tauri_plugin_window_state::Builder::default()
                // Visibility is driven by the first render, not by the last run.
                .with_state_flags(
                    StateFlags::SIZE
                        | StateFlags::POSITION
                        | StateFlags::MAXIMIZED
                        | StateFlags::FULLSCREEN
                        | StateFlags::DECORATIONS,
                )
                .build(),
        )
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .on_menu_event(handle_menu)
        .on_window_event(|window, event| {
            #[cfg(target_os = "macos")]
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                // A reader should reopen where it left off; the dock icon
                // (RunEvent::Reopen) and menu actions bring the window back.
                api.prevent_close();
                let _ = window.hide();
            }
            #[cfg(not(target_os = "macos"))]
            {
                let _ = (window, event);
            }
        })
        .setup(move |app| {
            let config = config_dir(app.handle()).unwrap_or_else(|_| std::env::temp_dir());
            let _ = std::fs::create_dir_all(&config);
            let themes_dir = config.join("themes");
            let _ = std::fs::create_dir_all(&themes_dir);

            let theme_set = ThemeSet::builtin().with_user_dirs(std::slice::from_ref(&themes_dir));
            let themes = theme_set.catalog();
            let recents = load_recents(app.handle());
            let state = AppState::new(themes, recents, themes_dir);
            if let Some(path) = initial_path {
                *state.initial_path.lock().unwrap() = Some(path);
            }
            app.manage(state);

            // Consume an early Finder/LaunchServices open now that state exists.
            #[cfg(target_os = "macos")]
            if let Some(path) = PENDING_OPEN.lock().unwrap().take() {
                *app.state::<AppState>().initial_path.lock().unwrap() = Some(path);
            }

            let menu = build_menu(app.handle())?;
            app.set_menu(menu)?;

            // If the frontend never finishes booting, still show the window
            // rather than leaving an invisible process behind.
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(1400));
                if let Some(window) = handle.get_webview_window("main") {
                    let _ = window.show();
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            bootstrap,
            show_window,
            start_window_drag,
            open_html_preview,
            html_preview_source,
            set_title,
            render_document,
            open_file_dialog,
            open_sample,
            read_image,
            open_external,
            open_path,
            reveal_themes,
            markdown_default_status,
            set_markdown_default
        ]);

    builder
        .build(tauri::generate_context!())
        .expect("error while building Folio")
        .run(|app, event| {
            #[cfg(target_os = "macos")]
            match event {
                tauri::RunEvent::Opened { urls } => {
                    if let Some(path) = urls.iter().find_map(|url| url.to_file_path().ok()) {
                        register_open(app, path);
                    }
                }
                tauri::RunEvent::Reopen { .. } => focus_main_window(app),
                _ => {}
            }
            #[cfg(not(target_os = "macos"))]
            {
                let _ = (app, event);
            }
        });
}

/* --------------------------------------------------------------------------
LaunchServices: default Markdown app
-------------------------------------------------------------------------- */

#[cfg(target_os = "macos")]
mod macos_default {
    use super::DefaultAppStatus;
    use std::ffi::{c_char, c_void, CString};

    type CFStringRef = *const c_void;
    type CFAllocatorRef = *const c_void;
    type CFIndex = isize;
    type Boolean = u8;

    const UTF8: u32 = 0x0800_0100;
    // kLSRolesAll
    const ROLE_ALL: u32 = 0xFFFF_FFFF;
    const PRIMARY_EXTENSION: &str = "md";
    const SECONDARY_EXTENSION: &str = "markdown";

    #[link(name = "CoreFoundation", kind = "framework")]
    #[link(name = "CoreServices", kind = "framework")]
    #[link(name = "UniformTypeIdentifiers", kind = "framework")]
    extern "C" {
        fn CFStringCreateWithCString(
            alloc: CFAllocatorRef,
            c_str: *const c_char,
            encoding: u32,
        ) -> CFStringRef;
        fn CFStringGetCString(
            the_string: CFStringRef,
            buffer: *mut c_char,
            buffer_size: CFIndex,
            encoding: u32,
        ) -> Boolean;
        fn CFStringGetLength(the_string: CFStringRef) -> CFIndex;
        fn CFRelease(cf: *const c_void);

        fn UTTypeCreatePreferredIdentifierForTag(
            tag_class: CFStringRef,
            tag: CFStringRef,
            conforming_to: CFStringRef,
        ) -> CFStringRef;

        fn LSSetDefaultRoleHandlerForContentType(
            content_type: CFStringRef,
            role_mask: u32,
            handler: CFStringRef,
        ) -> i32;
        fn LSCopyDefaultRoleHandlerForContentType(
            content_type: CFStringRef,
            role_mask: u32,
        ) -> CFStringRef;
    }

    fn is_app_bundle() -> bool {
        std::env::current_exe()
            .map(|path| path.to_string_lossy().contains(".app/Contents/MacOS/"))
            .unwrap_or(false)
    }

    fn cf_string(value: &str) -> CFStringRef {
        let Ok(c_string) = CString::new(value) else {
            return std::ptr::null();
        };
        unsafe { CFStringCreateWithCString(std::ptr::null(), c_string.as_ptr(), UTF8) }
    }

    fn cf_release(value: CFStringRef) {
        if !value.is_null() {
            unsafe { CFRelease(value) };
        }
    }

    fn cf_to_string(value: CFStringRef) -> Option<String> {
        if value.is_null() {
            return None;
        }
        let length = unsafe { CFStringGetLength(value) };
        if length <= 0 {
            return None;
        }
        let size = length.saturating_mul(4).saturating_add(1);
        let mut buffer = vec![0u8; size as usize];
        let ok =
            unsafe { CFStringGetCString(value, buffer.as_mut_ptr() as *mut c_char, size, UTF8) };
        if ok == 0 {
            return None;
        }
        let end = buffer
            .iter()
            .position(|byte| *byte == 0)
            .unwrap_or(buffer.len());
        String::from_utf8(buffer[..end].to_vec()).ok()
    }

    fn uti_for_extension(extension: &str) -> CFStringRef {
        let class = cf_string("public.filename-extension");
        let tag = cf_string(extension);
        if class.is_null() || tag.is_null() {
            cf_release(class);
            cf_release(tag);
            return std::ptr::null();
        }
        let uti = unsafe { UTTypeCreatePreferredIdentifierForTag(class, tag, std::ptr::null()) };
        cf_release(class);
        cf_release(tag);
        uti
    }

    fn is_default_for(extension: &str, bundle_id: &str) -> Option<bool> {
        let uti = uti_for_extension(extension);
        if uti.is_null() {
            return None;
        }
        let current = unsafe { LSCopyDefaultRoleHandlerForContentType(uti, ROLE_ALL) };
        cf_release(uti);
        if current.is_null() {
            return Some(false);
        }
        let matches = cf_to_string(current).as_deref() == Some(bundle_id);
        cf_release(current);
        Some(matches)
    }

    fn set_for(extension: &str, handler: CFStringRef) -> Result<(), String> {
        let uti = uti_for_extension(extension);
        if uti.is_null() {
            return Err(format!("Could not resolve the type for .{extension}."));
        }
        let status = unsafe { LSSetDefaultRoleHandlerForContentType(uti, ROLE_ALL, handler) };
        cf_release(uti);
        if status == 0 {
            Ok(())
        } else {
            Err(format!(
                "LaunchServices returned {status} while setting .{extension}."
            ))
        }
    }

    pub fn status(bundle_id: &str) -> DefaultAppStatus {
        if bundle_id.is_empty() || !is_app_bundle() {
            return DefaultAppStatus {
                available: false,
                is_default: false,
            };
        }
        let is_default = is_default_for(PRIMARY_EXTENSION, bundle_id).unwrap_or(false);
        DefaultAppStatus {
            available: true,
            is_default,
        }
    }

    pub fn set(bundle_id: &str) -> Result<(), String> {
        if bundle_id.is_empty() || !is_app_bundle() {
            return Err("Open the built Folio.app to set it as the default.".into());
        }
        let handler = cf_string(bundle_id);
        if handler.is_null() {
            return Err("Could not read the Folio bundle identifier.".into());
        }
        // `.md` is the promise; `.markdown` is a courtesy.
        let primary = set_for(PRIMARY_EXTENSION, handler);
        let _ = set_for(SECONDARY_EXTENSION, handler);
        cf_release(handler);
        primary
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png_signature() -> Vec<u8> {
        let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
        bytes.extend_from_slice(&[0; 16]);
        bytes
    }

    #[test]
    fn detects_raster_image_types() {
        assert_eq!(image_mime(&png_signature()), Some("image/png"));
        assert_eq!(
            image_mime(&[0xff, 0xd8, 0xff, 0xdb, 0x00]),
            Some("image/jpeg")
        );
        assert_eq!(image_mime(b"GIF89a...."), Some("image/gif"));

        let mut webp = b"RIFF".to_vec();
        webp.extend_from_slice(&[0, 0, 0, 0]);
        webp.extend_from_slice(b"WEBPVP8 ");
        assert_eq!(image_mime(&webp), Some("image/webp"));

        let mut avif = b"\0\0\0\x1cftyp".to_vec();
        avif.extend_from_slice(b"avif");
        assert_eq!(image_mime(&avif), Some("image/avif"));

        assert_eq!(
            image_mime(b"<svg xmlns=\"http://www.w3.org/2000/svg\">"),
            None
        );
        assert_eq!(image_mime(b"not an image"), None);
    }

    #[test]
    fn only_markdown_and_text_extensions_are_supported() {
        for name in [
            "a.md",
            "b.MARKDOWN",
            "c.mdown",
            "d.mkd",
            "e.mkdn",
            "f.mdx",
            "g.txt",
        ] {
            assert!(supported(Path::new(name)), "{name}");
        }
        for name in ["a.png", "b.pdf", "c.html", "d"] {
            assert!(!supported(Path::new(name)), "{name}");
        }
    }

    #[test]
    fn file_urls_and_plain_paths_are_accepted() {
        let plain = path_from_input("/tmp/note.md").unwrap();
        assert_eq!(plain, PathBuf::from("/tmp/note.md"));

        let url = path_from_input("file:///tmp/note%20one.md").unwrap();
        assert_eq!(url, PathBuf::from("/tmp/note one.md"));

        assert!(path_from_input("  ").is_err());
        assert!(path_from_input("file:///tmp/%ZZ").is_err());
    }

    #[test]
    fn loads_a_markdown_document_with_metadata() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("note.md");
        std::fs::write(
            &path,
            "---\ntitle: A Note\nauthor: Ada\n---\n\n# A Note\n\nThree quiet words.\n",
        )
        .unwrap();

        let payload = load_document(&path).unwrap();
        assert_eq!(payload.name, "note.md");
        assert!(payload.path.ends_with("note.md"));
        assert_eq!(payload.meta.title.as_deref(), Some("A Note"));
        assert_eq!(payload.meta.author.as_deref(), Some("Ada"));
        assert!(payload.html.contains("Three quiet words."));
        assert!(!payload.sample);
        assert!(payload.size > 0);
        assert!(payload.modified_ms > 0);
    }

    #[test]
    fn rejects_unsupported_documents() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("note.png");
        std::fs::write(&path, b"\x89PNG\r\n\x1a\n").unwrap();
        let error = load_document(&path).unwrap_err();
        assert!(error.contains("Markdown and plain text"));

        let missing = load_document(&dir.path().join("missing.md")).unwrap_err();
        assert!(!missing.is_empty());
    }

    #[test]
    fn preview_titles_are_single_line_and_bounded() {
        assert_eq!(preview_title(None), "HTML Preview — Folio");
        assert_eq!(preview_title(Some("Hello\nworld")), "Hello — Folio");
        assert_eq!(preview_title(Some("  ")), "HTML Preview — Folio");
        let long = preview_title(Some(&"x".repeat(200)));
        assert!(long.ends_with(" — Folio"));
        assert!(long.chars().count() <= 88);
    }

    #[test]
    fn timestamps_are_plausible() {
        assert!(now_ms() > 1_700_000_000_000);
    }
}
