//! Tool implementations.
//! Metadata, files and AI run in Rust through the functions the app's commands use (saving goes through the Save button's `save_metadata` path).
//! The window answers for what only it holds (the open folder, the selection, settings, the AI configuration); before a change it keeps the editor's pending edits, and afterwards it shows the result.

use std::collections::{HashMap, HashSet};
use std::ffi::{OsStr, OsString};
use std::future::Future;
use std::path::{Component, Path, PathBuf, Prefix};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::{json, Map, Value};
use tauri::{AppHandle, Manager};

use super::catalog::{
    DEFAULT_WAIT_SECS, LARGE_PREVIEW_EDGE, MAX_JOB_PATHS, MAX_LARGE_PREVIEWS, MAX_PATHS,
    MAX_SMALL_PREVIEWS, MAX_WAIT_SECS, PROVIDER_IDS, SMALL_PREVIEW_EDGE,
};
use super::http::ServerHost;
use super::jobs::Job;
use super::metadata::{self, SetMetadataArgs, CONFLICT};
use super::protocol::{tool_error, tool_json, CallContext, ToolHost};
use super::ui::{self, UiError};
use super::McpState;
use crate::ai::types::{AttributionConfig, AttributionResult};
use crate::store::{DbState, MetadataResolution, StoredMetadata};
use crate::types::SaveRequest;

/// Time the window has to open a folder; a larger folder keeps opening after the call returns.
const OPEN_FOLDER_TIMEOUT: Duration = Duration::from_secs(50);
/// Time a provider has to list its models.
const MODELS_TIMEOUT: Duration = Duration::from_secs(50);
/// How often a queued attribution job checks for cancellation while the running one holds the AI.
const QUEUE_POLL: Duration = Duration::from_millis(250);

/// Serves tool calls inside the app.
pub(crate) struct AppHost {
    app: AppHandle,
}

impl AppHost {
    pub(crate) fn new(app: AppHandle) -> Self {
        Self { app }
    }
}

impl ToolHost for AppHost {
    fn call_tool(
        &self,
        name: &str,
        arguments: Value,
        ctx: CallContext,
    ) -> impl Future<Output = Value> + Send {
        let app = self.app.clone();
        let name = name.to_string();
        async move { run(&app, &name, arguments, ctx).await }
    }
}

impl ServerHost for AppHost {
    fn token(&self) -> String {
        self.app.state::<McpState>().config().token
    }
}

/// What a tool produced: structured JSON, optionally followed by extra content blocks (previews).
enum Output {
    Json(Value),
    WithContent(Value, Vec<Value>),
}

type ToolResult = Result<Output, String>;

async fn run(app: &AppHandle, name: &str, arguments: Value, ctx: CallContext) -> Value {
    let started = Instant::now();
    let outcome = dispatch(app, name, arguments).await;
    let (result, error) = match outcome {
        Ok(Output::Json(value)) => (tool_json(value), None),
        Ok(Output::WithContent(value, blocks)) => {
            let mut result = tool_json(value);
            if let Some(content) = result["content"].as_array_mut() {
                content.extend(blocks);
            }
            (result, None)
        }
        Err(e) => (tool_error(e.clone()), Some(e)),
    };
    app.state::<McpState>()
        .record(name, ctx.client, error, started.elapsed());
    result
}

async fn dispatch(app: &AppHandle, name: &str, args: Value) -> ToolResult {
    match name {
        "get_status" => get_status(app).await,
        "list_images" => list_images(app, parse(args)?).await,
        "get_metadata" => get_metadata(app, parse(args)?).await,
        "get_preview" => get_preview(parse(args)?).await,
        "open_folder" => open_folder(app, parse(args)?).await,
        "select_images" => select_images(app, parse(args)?).await,
        "set_metadata" => set_metadata(app, parse(args)?).await,
        "save_metadata" => save_metadata(app, parse(args)?).await,
        "revert_metadata" => revert_metadata(app, parse(args)?).await,
        "resolve_conflict" => resolve_conflict(app, parse(args)?).await,
        "attribute_images" => attribute_images(app, parse(args)?).await,
        "get_job" => get_job(app, parse(args)?).await,
        "cancel_job" => cancel_job(app, parse(args)?),
        "get_settings" => get_settings(app).await,
        "update_settings" => update_settings(app, parse(args)?).await,
        "list_ai_models" => list_ai_models(app, parse(args)?).await,
        _ => Err(format!("unknown tool {name}")),
    }
}

fn parse<T: DeserializeOwned>(args: Value) -> Result<T, String> {
    serde_json::from_value(args).map_err(|e| format!("invalid arguments: {e}"))
}

// ── Arguments ─────────────────────────────────────────────────────────────

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PathsArgs {
    paths: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ListArgs {
    #[serde(default)]
    offset: usize,
    limit: Option<usize>,
}

#[derive(Deserialize, Default, Clone, Copy)]
#[serde(rename_all = "lowercase")]
enum PreviewSize {
    #[default]
    Small,
    Large,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PreviewArgs {
    paths: Vec<String>,
    #[serde(default)]
    size: PreviewSize,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FolderArgs {
    path: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SaveArgs {
    paths: Vec<String>,
    file_name: Option<String>,
    wait_seconds: Option<u64>,
}

#[derive(Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum Keep {
    App,
    File,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ResolveArgs {
    paths: Vec<String>,
    keep: Keep,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AttributeArgs {
    paths: Vec<String>,
    wait_seconds: Option<u64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct JobArgs {
    job_id: String,
    wait_seconds: Option<u64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SettingsArgs {
    values: Map<String, Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ModelsArgs {
    provider: Option<String>,
}

// ── Paths ─────────────────────────────────────────────────────────────────

/// Directory listings read while spelling paths, by directory.
type DirCache = HashMap<PathBuf, Vec<OsString>>;

/// `path` spelled as the app keys it: on Windows with backslashes and an upper-case drive letter.
/// Windows and macOS file systems ignore case, so every existing component also takes its on-disk case; otherwise a path typed in another case would get a metadata record of its own and miss the window's state.
/// Blocking (reads directories).
fn app_spelling(path: &str, dirs: &mut DirCache) -> String {
    let path = path.trim();
    let path = if cfg!(windows) {
        path.replace('/', "\\")
    } else {
        path.to_string()
    };
    if !cfg!(any(windows, target_os = "macos")) {
        return path;
    }
    let mut out = PathBuf::new();
    let mut names: Vec<&OsStr> = Vec::new();
    for component in Path::new(&path).components() {
        match component {
            Component::Prefix(prefix) => match prefix.kind() {
                Prefix::Disk(letter) | Prefix::VerbatimDisk(letter) => {
                    out.push(format!("{}:", char::from(letter).to_ascii_uppercase()))
                }
                _ => out.push(prefix.as_os_str()),
            },
            Component::RootDir => out.push(component.as_os_str()),
            Component::Normal(name) => names.push(name),
            // Relative and dotted paths are left as given; the absolute-path checks refuse them.
            Component::CurDir | Component::ParentDir => return path,
        }
    }
    let mut exists = true;
    for name in names {
        let actual = if exists {
            let listing = dirs.entry(out.clone()).or_insert_with(|| list_dir(&out));
            on_disk_name(listing, name)
        } else {
            None
        };
        exists = actual.is_some();
        out.push(actual.unwrap_or_else(|| name.to_os_string()));
    }
    out.to_string_lossy().into_owned()
}

fn list_dir(dir: &Path) -> Vec<OsString> {
    std::fs::read_dir(dir)
        .map(|entries| entries.flatten().map(|e| e.file_name()).collect())
        .unwrap_or_default()
}

/// The entry of `listing` that is `name` ignoring case, preferring an exact match.
fn on_disk_name(listing: &[OsString], name: &OsStr) -> Option<OsString> {
    if listing.iter().any(|e| e == name) {
        return Some(name.to_os_string());
    }
    let wanted = name.to_string_lossy().to_lowercase();
    listing
        .iter()
        .find(|e| e.to_string_lossy().to_lowercase() == wanted)
        .cloned()
}

/// The distinct paths of a call in their given order, spelled as the app keys them.
///
/// # Errors
///
/// Returns a message when there are none or more than `max`.
async fn paths_arg(paths: Vec<String>, max: usize) -> Result<Vec<String>, String> {
    let mut typed = HashSet::new();
    let given: Vec<String> = paths
        .into_iter()
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty() && typed.insert(p.clone()))
        .collect();
    if given.is_empty() {
        return Err("paths must name at least one image".into());
    }
    if given.len() > max {
        return Err(format!("at most {max} images per call; split the list"));
    }
    let spelled = blocking(move || {
        let mut dirs = DirCache::new();
        given
            .iter()
            .map(|p| app_spelling(p, &mut dirs))
            .collect::<Vec<_>>()
    })
    .await?;
    let mut seen = HashSet::new();
    Ok(spelled
        .into_iter()
        .filter(|p| seen.insert(p.clone()))
        .collect())
}

/// Checks that `path` names an existing image file Attributor edits. Blocking.
fn check_image(path: &str) -> Result<(), String> {
    let p = Path::new(path);
    if !p.is_absolute() {
        return Err("the path must be absolute".into());
    }
    if !crate::folder::is_supported_image(p) {
        return Err("not a JPEG, PNG or WebP file".into());
    }
    if !p.is_file() {
        return Err("the file does not exist".into());
    }
    Ok(())
}

// ── Helpers ───────────────────────────────────────────────────────────────

fn failed(path: &str, error: impl Into<String>) -> Value {
    json!({"path": path, "ok": false, "error": error.into()})
}

fn cancelled(path: &str) -> Value {
    failed(path, "cancelled")
}

fn wait_for(seconds: Option<u64>, default: u64) -> Duration {
    Duration::from_secs(seconds.unwrap_or(default).min(MAX_WAIT_SECS))
}

fn store(app: &AppHandle) -> Result<DbState, String> {
    app.try_state::<DbState>()
        .map(|db| db.share())
        .ok_or_else(|| "Attributor is still starting; try again in a moment".to_string())
}

/// A store that can keep changes; without it, edits made through MCP would be lost.
fn writable_store(app: &AppHandle) -> Result<DbState, String> {
    let db = store(app)?;
    if db.is_available() {
        Ok(db)
    } else {
        Err("Attributor's metadata store is unavailable, so changes cannot be kept; see the app log".into())
    }
}

async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> T + Send + 'static,
) -> Result<T, String> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|e| e.to_string())
}

async fn ask(app: &AppHandle, op: &str, params: Value, timeout: Duration) -> Result<Value, String> {
    app.state::<McpState>()
        .ui
        .call(app, op, params, timeout)
        .await
        .map_err(String::from)
}

/// Asks the window to keep the editor's pending edits of `paths`; `guard` also pauses the folder rescans while many files are written.
async fn prepare(app: &AppHandle, paths: &[String], guard: bool) {
    let state = app.state::<McpState>();
    if !state.ui.is_ready() {
        return;
    }
    let params = json!({"paths": paths, "guard": guard});
    if let Err(e) = state
        .ui
        .call(app, "prepare", params, ui::DEFAULT_TIMEOUT)
        .await
    {
        log::warn!(
            "mcp: the window could not keep pending edits: {}",
            String::from(e)
        );
    }
}

/// What the window should show again after a change.
#[derive(Default)]
struct Shown<'a> {
    paths: &'a [String],
    renamed: &'a [(String, String)],
    /// Ends a [`prepare`] guard.
    release: bool,
    /// Also reload the batch view when it holds one of the paths.
    batch: bool,
}

/// Asks the window to show the new state of changed images (and to follow renamed files).
async fn refresh(app: &AppHandle, shown: Shown<'_>) {
    let state = app.state::<McpState>();
    if !state.ui.is_ready() {
        return;
    }
    let renamed: Vec<Value> = shown
        .renamed
        .iter()
        .map(|(from, to)| json!({"from": from, "to": to}))
        .collect();
    let params = json!({"paths": shown.paths, "renamed": renamed, "release": shown.release, "batch": shown.batch});
    if let Err(e) = state
        .ui
        .call(app, "refresh", params, ui::DEFAULT_TIMEOUT)
        .await
    {
        log::warn!("mcp: the window could not refresh: {}", String::from(e));
    }
}

/// Whether `key` belongs to the AI settings, which need the user's permission.
fn is_ai_key(key: &str) -> bool {
    key.starts_with("ai.")
        || key
            .split_once('.')
            .is_some_and(|(prefix, _)| PROVIDER_IDS.contains(&prefix))
}

// ── Reading ───────────────────────────────────────────────────────────────

async fn get_status(app: &AppHandle) -> ToolResult {
    let allow_ai = app.state::<McpState>().config().allow_ai_settings;
    let mut status = json!({
        "app": "Attributor",
        "version": env!("CARGO_PKG_VERSION"),
        "aiSettingsWritable": allow_ai,
    });
    match ask(app, "status", json!({}), ui::DEFAULT_TIMEOUT).await {
        Ok(Value::Object(window)) => {
            if let Some(obj) = status.as_object_mut() {
                obj.extend(window);
            }
        }
        Ok(_) => {}
        Err(e) => status["window"] = json!(e),
    }
    Ok(Output::Json(status))
}

async fn list_images(app: &AppHandle, args: ListArgs) -> ToolResult {
    let listing = ask(app, "list_images", json!({}), ui::DEFAULT_TIMEOUT).await?;
    let Some(folder) = listing["folder"].as_str() else {
        return Err("no folder is open in Attributor; call open_folder first".into());
    };
    let images: Vec<String> = serde_json::from_value(listing["images"].clone()).unwrap_or_default();
    let limit = args.limit.unwrap_or(200).clamp(1, 1000);
    let page: Vec<Value> = images
        .iter()
        .skip(args.offset)
        .take(limit)
        .map(|p| json!({"path": p, "name": Path::new(p).file_name().map(|n| n.to_string_lossy()).unwrap_or_default()}))
        .collect();
    Ok(Output::Json(json!({
        "folder": folder,
        "total": images.len(),
        "offset": args.offset,
        "hasMore": args.offset.saturating_add(page.len()) < images.len(),
        "images": page,
    })))
}

async fn get_metadata(app: &AppHandle, args: PathsArgs) -> ToolResult {
    let paths = paths_arg(args.paths, MAX_PATHS).await?;
    prepare(app, &paths, false).await;
    let db = store(app)?;
    let images = blocking(move || {
        paths
            .iter()
            .map(|p| match check_image(p).and_then(|()| db.resolve(p)) {
                Ok(resolution) => metadata::resolution_json(p, &resolution),
                Err(e) => failed(p, e),
            })
            .collect::<Vec<_>>()
    })
    .await?;
    Ok(Output::Json(json!({"images": images})))
}

async fn get_preview(args: PreviewArgs) -> ToolResult {
    let (edge, max, size) = match args.size {
        PreviewSize::Small => (SMALL_PREVIEW_EDGE, MAX_SMALL_PREVIEWS, "small"),
        PreviewSize::Large => (LARGE_PREVIEW_EDGE, MAX_LARGE_PREVIEWS, "large"),
    };
    let paths = paths_arg(args.paths, max).await?;
    let made = futures_util::future::join_all(paths.into_iter().map(|path| async move {
        let source = path.clone();
        let image = blocking(move || {
            check_image(&source)?;
            crate::ai::image::prepare_jpeg(&source, edge)
        })
        .await
        .and_then(|r| r);
        (path, image)
    }))
    .await;
    let mut blocks = Vec::new();
    let mut previews = Vec::new();
    let mut errors = Vec::new();
    for (path, image) in made {
        match image {
            Ok(image) => {
                blocks.push(json!({"type": "text", "text": path}));
                blocks.push(json!({"type": "image", "data": image.base64, "mimeType": image.mime}));
                previews.push(json!({"path": path, "mimeType": image.mime}));
            }
            Err(e) => errors.push(failed(&path, e)),
        }
    }
    if previews.is_empty() {
        return Err(format!(
            "no preview could be made: {}",
            Value::Array(errors)
        ));
    }
    Ok(Output::WithContent(
        json!({"size": size, "previews": previews, "errors": errors}),
        blocks,
    ))
}

// ── The window ────────────────────────────────────────────────────────────

async fn open_folder(app: &AppHandle, args: FolderArgs) -> ToolResult {
    let path = blocking(move || {
        let path = app_spelling(&args.path, &mut DirCache::new());
        let dir = Path::new(&path);
        if !dir.is_absolute() {
            return Err("the path must be absolute".to_string());
        }
        if !dir.is_dir() {
            return Err("the folder does not exist".to_string());
        }
        Ok(path)
    })
    .await??;
    let state = app.state::<McpState>();
    let _changes = state.write_lock.lock().await;
    match state
        .ui
        .call(
            app,
            "open_folder",
            json!({"path": path}),
            OPEN_FOLDER_TIMEOUT,
        )
        .await
    {
        Ok(opened) => Ok(Output::Json(opened)),
        Err(UiError::Timeout(_)) => Ok(Output::Json(json!({
            "folder": path,
            "status": "opening",
            "note": "The folder is large and still opening; call get_status or list_images in a moment.",
        }))),
        Err(e) => Err(e.into()),
    }
}

async fn select_images(app: &AppHandle, args: PathsArgs) -> ToolResult {
    let paths = paths_arg(args.paths, MAX_PATHS).await?;
    let selected = ask(app, "select", json!({"paths": paths}), ui::DEFAULT_TIMEOUT).await?;
    Ok(Output::Json(selected))
}

async fn get_settings(app: &AppHandle) -> ToolResult {
    let allow_ai = app.state::<McpState>().config().allow_ai_settings;
    let mut settings = ask(app, "get_settings", json!({}), ui::DEFAULT_TIMEOUT).await?;
    settings["aiSettingsWritable"] = json!(allow_ai);
    Ok(Output::Json(settings))
}

async fn update_settings(app: &AppHandle, args: SettingsArgs) -> ToolResult {
    if args.values.is_empty() {
        return Err("values must name at least one setting".into());
    }
    if let Some(key) = args.values.keys().find(|k| k.starts_with("mcp.")) {
        return Err(format!(
            "{key}: MCP settings can be changed only in Attributor itself"
        ));
    }
    let state = app.state::<McpState>();
    let _changes = state.write_lock.lock().await;
    // Read after the wait, so a permission the user withdrew meanwhile counts.
    let allow_ai = state.config().allow_ai_settings;
    if !allow_ai {
        let blocked: Vec<&str> = args
            .values
            .keys()
            .map(String::as_str)
            .filter(|k| is_ai_key(k))
            .collect();
        if !blocked.is_empty() {
            return Err(format!(
                "changing AI settings ({}) through MCP is turned off; the user can allow it in Attributor > Settings > MCP",
                blocked.join(", ")
            ));
        }
    }
    let params = json!({"values": args.values, "allowAi": allow_ai});
    let applied = ask(app, "update_settings", params, ui::DEFAULT_TIMEOUT).await?;
    Ok(Output::Json(applied))
}

async fn list_ai_models(app: &AppHandle, args: ModelsArgs) -> ToolResult {
    if let Some(provider) = &args.provider {
        if !PROVIDER_IDS.contains(&provider.as_str()) {
            return Err(format!(
                "unknown provider {provider}; use one of {}",
                PROVIDER_IDS.join(", ")
            ));
        }
    }
    let models = ask(
        app,
        "list_models",
        json!({"provider": args.provider}),
        MODELS_TIMEOUT,
    )
    .await?;
    Ok(Output::Json(models))
}

// ── Changing metadata ─────────────────────────────────────────────────────

async fn set_metadata(app: &AppHandle, mut args: SetMetadataArgs) -> ToolResult {
    if args.changes_nothing() {
        return Err("pass at least one field to change: title, description, keywords, categories, releaseFilename, editorial, matureContent or illustration".into());
    }
    let paths = paths_arg(std::mem::take(&mut args.paths), MAX_PATHS).await?;
    let state = app.state::<McpState>();
    let _changes = state.write_lock.lock().await;
    prepare(app, &paths, false).await;
    let db = writable_store(app)?;
    let (images, changed) = blocking(move || {
        let mut changed = Vec::new();
        let images: Vec<Value> = paths
            .iter()
            .map(|p| set_one(&db, p, &args, &mut changed))
            .collect();
        (images, changed)
    })
    .await?;
    refresh(
        app,
        Shown {
            paths: &changed,
            batch: true,
            ..Shown::default()
        },
    )
    .await;
    Ok(Output::Json(json!({"images": images})))
}

fn set_one(db: &DbState, path: &str, args: &SetMetadataArgs, changed: &mut Vec<String>) -> Value {
    let (current, sync) = match check_image(path).and_then(|()| db.resolve(path)) {
        Ok(MetadataResolution::Resolved {
            metadata,
            sync_state,
        }) => (metadata, sync_state),
        Ok(MetadataResolution::Conflict { .. }) => return failed(path, CONFLICT),
        Err(e) => return failed(path, e),
    };
    let updated = args.apply(&current);
    if updated == current {
        return json!({"path": path, "ok": true, "changed": false, "status": metadata::status_name(sync), "metadata": metadata::metadata_json(path, &updated)});
    }
    match db.persist(path, &updated) {
        Ok(sync) => {
            changed.push(path.to_string());
            json!({"path": path, "ok": true, "changed": true, "status": metadata::status_name(sync), "metadata": metadata::metadata_json(path, &updated)})
        }
        Err(e) => failed(path, e),
    }
}

async fn revert_metadata(app: &AppHandle, args: PathsArgs) -> ToolResult {
    let paths = paths_arg(args.paths, MAX_PATHS).await?;
    let state = app.state::<McpState>();
    let _changes = state.write_lock.lock().await;
    prepare(app, &paths, false).await;
    let db = writable_store(app)?;
    let (images, changed) = blocking(move || {
        let mut changed = Vec::new();
        let images: Vec<Value> = paths
            .iter()
            .map(|p| match check_image(p).and_then(|()| db.revert(p)) {
                Ok(meta) => {
                    changed.push(p.clone());
                    json!({"path": p, "ok": true, "status": "synced", "metadata": metadata::metadata_json(p, &meta)})
                }
                Err(e) => failed(p, e),
            })
            .collect();
        (images, changed)
    })
    .await?;
    refresh(
        app,
        Shown {
            paths: &changed,
            batch: true,
            ..Shown::default()
        },
    )
    .await;
    Ok(Output::Json(json!({"images": images})))
}

async fn resolve_conflict(app: &AppHandle, args: ResolveArgs) -> ToolResult {
    let paths = paths_arg(args.paths, MAX_PATHS).await?;
    let from_file = args.keep == Keep::File;
    let state = app.state::<McpState>();
    let _changes = state.write_lock.lock().await;
    prepare(app, &paths, false).await;
    let db = writable_store(app)?;
    let (images, changed) = blocking(move || {
        let mut changed = Vec::new();
        let images: Vec<Value> = paths
            .iter()
            .map(|p| resolve_one(&db, p, from_file, &mut changed))
            .collect();
        (images, changed)
    })
    .await?;
    refresh(
        app,
        Shown {
            paths: &changed,
            batch: true,
            ..Shown::default()
        },
    )
    .await;
    Ok(Output::Json(json!({"images": images})))
}

/// Resolves the conflict of one image as the editor's conflict prompt does; an image without a conflict is left alone.
fn resolve_one(db: &DbState, path: &str, from_file: bool, changed: &mut Vec<String>) -> Value {
    match check_image(path).and_then(|()| db.resolve(path)) {
        Ok(MetadataResolution::Resolved { sync_state, .. }) => failed(
            path,
            format!(
                "no conflict to resolve; the image is {}",
                metadata::status_name(sync_state)
            ),
        ),
        Ok(MetadataResolution::Conflict { .. }) => match db.apply_source(path, from_file) {
            Ok(resolution) => {
                changed.push(path.to_string());
                let mut item = metadata::resolution_json(path, &resolution);
                item["ok"] = json!(true);
                item
            }
            Err(e) => failed(path, e),
        },
        Err(e) => failed(path, e),
    }
}

// ── Jobs ──────────────────────────────────────────────────────────────────

async fn save_metadata(app: &AppHandle, args: SaveArgs) -> ToolResult {
    let file_name = args
        .file_name
        .as_deref()
        .map(metadata::clean_file_name)
        .transpose()?;
    let paths = paths_arg(args.paths, MAX_JOB_PATHS).await?;
    if file_name.is_some() && paths.len() != 1 {
        return Err("fileName renames one file; pass a single path with it".into());
    }
    store(app)?;
    let wait = wait_for(args.wait_seconds, DEFAULT_WAIT_SECS);
    let worker = app.clone();
    let job = app
        .state::<McpState>()
        .jobs
        .spawn("save_metadata", paths.len(), move |job| {
            save_job(worker, job, paths, file_name)
        });
    Ok(Output::Json(job.wait(wait).await))
}

async fn save_job(app: AppHandle, job: Arc<Job>, paths: Vec<String>, file_name: Option<String>) {
    // Several files: pause the folder rescans their writes would trigger, as the batch Save does. A single save may rename the file, which the rescan must pick up.
    let many = paths.len() > 1;
    let state = app.state::<McpState>();
    let mut touched = Vec::new();
    let mut renamed = Vec::new();
    for path in &paths {
        if job.is_cancelled() {
            job.push(cancelled(path));
            continue;
        }
        let saved = {
            let _changes = state.write_lock.lock().await;
            prepare(&app, std::slice::from_ref(path), many).await;
            save_one(&app, path, file_name.clone()).await
        };
        match saved {
            Ok(saved) => {
                if &saved != path {
                    renamed.push((path.clone(), saved.clone()));
                }
                job.push(json!({"path": path, "ok": true, "savedAs": saved}));
                touched.push(saved);
            }
            Err(e) => job.push(failed(path, e)),
        }
    }
    refresh(
        &app,
        Shown {
            paths: &touched,
            renamed: &renamed,
            release: many,
            batch: true,
        },
    )
    .await;
}

/// Saves one image like the editor's Save button: the record the app holds, the editor's checks, then the button's `save_metadata` path.
async fn save_one(
    app: &AppHandle,
    path: &str,
    file_name: Option<String>,
) -> Result<String, String> {
    let db = store(app)?;
    let path = path.to_string();
    blocking(move || {
        check_image(&path)?;
        let meta = match db.resolve(&path)? {
            MetadataResolution::Resolved { metadata, .. } => metadata,
            MetadataResolution::Conflict { .. } => return Err(CONFLICT.to_string()),
        };
        let stem = file_name.unwrap_or_else(|| metadata::stem_of(&path));
        metadata::check_before_save(&stem, &meta)?;
        let request = SaveRequest {
            filepath: path,
            filename: stem,
            title: meta.title,
            description: meta.description,
            keywords: meta.keywords,
            categories: meta.categories,
            release_filename: meta.release_filename,
            editorial: meta.editorial,
            mature_content: meta.mature_content,
            illustration: meta.illustration,
        };
        crate::save_metadata_impl(request, &db)
    })
    .await?
}

async fn attribute_images(app: &AppHandle, args: AttributeArgs) -> ToolResult {
    let paths = paths_arg(args.paths, MAX_JOB_PATHS).await?;
    writable_store(app)?;
    let setup = ask(app, "attribution_config", json!({}), ui::DEFAULT_TIMEOUT).await?;
    if !setup["available"].as_bool().unwrap_or(false) {
        let reason = setup["reason"]
            .as_str()
            .unwrap_or("the AI provider is not set up");
        return Err(format!("AI attribution is not available: {reason}. The user can set it up in Attributor > Settings > AI attribution"));
    }
    let config: AttributionConfig = serde_json::from_value(setup["config"].clone())
        .map_err(|e| format!("invalid AI configuration: {e}"))?;
    let wait = wait_for(args.wait_seconds, DEFAULT_WAIT_SECS);
    let worker = app.clone();
    let job = app
        .state::<McpState>()
        .jobs
        .spawn("attribute_images", paths.len(), move |job| {
            attribute_job(worker, job, paths, config)
        });
    let mut snapshot = job.wait(wait).await;
    snapshot["provider"] = setup["providerLabel"].clone();
    snapshot["model"] = setup["model"].clone();
    Ok(Output::Json(snapshot))
}

async fn attribute_job(
    app: AppHandle,
    job: Arc<Job>,
    paths: Vec<String>,
    config: AttributionConfig,
) {
    let state = app.state::<McpState>();
    // One attribution job at a time; a queued job still notices cancel_job while it waits.
    let queue = state.ai_lock.lock();
    tokio::pin!(queue);
    let _one_at_a_time = loop {
        tokio::select! {
            guard = &mut queue => break guard,
            () = tokio::time::sleep(QUEUE_POLL) => {
                if job.is_cancelled() {
                    for path in &paths {
                        job.push(cancelled(path));
                    }
                    return;
                }
            }
        }
    };
    let ai = app.state::<crate::ai::AiState>();
    let mut stored = Vec::new();
    for path in &paths {
        if job.is_cancelled() {
            job.push(cancelled(path));
            continue;
        }
        // Checked before the (possibly paid) inference: an image in conflict would have its outside changes overwritten.
        if let Err(e) = check_attributable(&app, path).await {
            job.push(failed(path, e));
            continue;
        }
        let item = match ai
            .attribute_detached(path, &config, job.cancel_flag())
            .await
        {
            Err(_) if job.is_cancelled() => cancelled(path),
            Err(e) => failed(path, e),
            Ok(result) => {
                let _changes = state.write_lock.lock().await;
                let one = std::slice::from_ref(path);
                prepare(&app, one, false).await;
                let kept = keep_attribution(&app, path, result).await;
                // Each image updates the editor right away; the batch view reloads once, after the job.
                refresh(
                    &app,
                    Shown {
                        paths: one,
                        ..Shown::default()
                    },
                )
                .await;
                match kept {
                    Ok(meta) => {
                        stored.push(path.clone());
                        json!({"path": path, "ok": true, "status": "appOnly", "metadata": metadata::metadata_json(path, &meta)})
                    }
                    Err(e) => failed(path, e),
                }
            }
        };
        job.push(item);
    }
    if !stored.is_empty() {
        refresh(
            &app,
            Shown {
                paths: &stored,
                batch: true,
                ..Shown::default()
            },
        )
        .await;
    }
}

/// Fails for an image that is not an editable file or whose file changed outside the app (a conflict).
async fn check_attributable(app: &AppHandle, path: &str) -> Result<(), String> {
    let db = store(app)?;
    let path = path.to_string();
    blocking(move || {
        check_image(&path)?;
        match db.resolve(&path)? {
            MetadataResolution::Conflict { .. } => Err(CONFLICT.to_string()),
            MetadataResolution::Resolved { .. } => Ok(()),
        }
    })
    .await?
}

/// Stores an attribution result as the batch attribution does (keywords merged, the release file name kept) and returns the stored record.
/// The conflict check is repeated here because the file may have changed during the inference.
async fn keep_attribution(
    app: &AppHandle,
    path: &str,
    result: AttributionResult,
) -> Result<StoredMetadata, String> {
    let db = writable_store(app)?;
    let model = crate::ai::attribute::stored_metadata(result);
    let path = path.to_string();
    blocking(move || {
        if let MetadataResolution::Conflict { .. } = db.resolve(&path)? {
            return Err(CONFLICT.to_string());
        }
        db.attribute(&path, &model)?;
        db.fetch(&path)
            .ok_or_else(|| "could not read the stored result back".to_string())
    })
    .await?
}

async fn get_job(app: &AppHandle, args: JobArgs) -> ToolResult {
    let job = find_job(app, &args.job_id)?;
    Ok(Output::Json(
        job.wait(wait_for(args.wait_seconds, 30)).await,
    ))
}

fn cancel_job(app: &AppHandle, args: JobArgs) -> ToolResult {
    let job = find_job(app, &args.job_id)?;
    job.cancel();
    Ok(Output::Json(job.snapshot()))
}

fn find_job(app: &AppHandle, id: &str) -> Result<Arc<Job>, String> {
    app.state::<McpState>()
        .jobs
        .get(id)
        .ok_or_else(|| format!("unknown job {id}; finished jobs are kept for an hour"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn paths_are_deduplicated_and_bounded() {
        let dir = std::env::temp_dir().to_string_lossy().into_owned();
        let a = format!("{dir}/attributor-a.jpg");
        let paths = vec![
            a.clone(),
            format!(" {a} "),
            format!("{dir}/attributor-b.jpg"),
            "".into(),
        ];
        assert_eq!(paths_arg(paths, 10).await.unwrap().len(), 2);
        assert!(paths_arg(vec![], 10).await.is_err());
        assert!(paths_arg(vec!["/a".into(), "/b".into()], 1).await.is_err());
        // Repeated paths do not count against the limit.
        assert_eq!(
            paths_arg(vec![a.clone(), a.clone()], 1)
                .await
                .unwrap()
                .len(),
            1
        );
    }

    #[cfg(any(windows, target_os = "macos"))]
    #[test]
    fn paths_take_the_on_disk_case() {
        let root = std::env::temp_dir().join(format!("Attributor-Case-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("Sub Dir")).unwrap();
        std::fs::write(root.join("Sub Dir").join("Photo.JPG"), b"x").unwrap();
        let typed = root
            .join("sub dir")
            .join("photo.jpg")
            .to_string_lossy()
            .to_lowercase();
        let spelled = app_spelling(&typed, &mut DirCache::new());
        assert!(
            spelled.ends_with(&format!("Sub Dir{}Photo.JPG", std::path::MAIN_SEPARATOR)),
            "{spelled}"
        );
        // A missing tail keeps the typed spelling.
        let missing = root
            .join("Sub Dir")
            .join("new.jpg")
            .to_string_lossy()
            .to_lowercase();
        assert!(app_spelling(&missing, &mut DirCache::new()).ends_with("new.jpg"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[cfg(windows)]
    #[test]
    fn windows_separators_and_drive_letters_are_normalized() {
        let spelled = app_spelling(
            " c:/windows/SYSTEM32/missing-attributor-file.jpg ",
            &mut DirCache::new(),
        );
        assert!(spelled.starts_with(r"C:\Windows\System32\"), "{spelled}");
        assert!(
            spelled.ends_with(r"\missing-attributor-file.jpg"),
            "{spelled}"
        );
    }

    #[test]
    fn ai_keys_need_permission() {
        assert!(is_ai_key("ai.provider"));
        assert!(is_ai_key("ollama.baseUrl"));
        assert!(is_ai_key("anthropic.activeModel"));
        assert!(!is_ai_key("editor.autosave"));
        assert!(!is_ai_key("general.language"));
    }

    #[test]
    fn image_paths_are_checked() {
        assert!(check_image("relative.jpg").is_err());
        let missing = std::env::temp_dir().join("attributor-missing-image.jpg");
        assert_eq!(
            check_image(&missing.to_string_lossy()).unwrap_err(),
            "the file does not exist"
        );
        let text = std::env::temp_dir().join("attributor-not-an-image.txt");
        assert!(check_image(&text.to_string_lossy())
            .unwrap_err()
            .contains("JPEG"));
    }
}
