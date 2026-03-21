use bytes::{Bytes, BytesMut};
use img_parts::jpeg::{markers, Jpeg, JpegSegment};
use img_parts::png::{Png, PngChunk};
use img_parts::riff::RiffContent;
use img_parts::webp::{WebP, CHUNK_XMP};
use img_parts::DynImage;
use log::{debug, error, info, warn};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use quick_xml::events::{BytesEnd, BytesStart, BytesText, Event};
use quick_xml::Reader;
use quick_xml::Writer;
use serde::{Deserialize, Serialize};
use std::io::Cursor;
use std::sync::Mutex;

// ── Request / response types ───────────────────────────────────────────────

/// Payload sent from the frontend when saving.
/// `filepath` is the current full path; `filename` is the desired stem
/// (no extension, no directory). If the stem changed, the file is renamed.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveRequest {
    pub filepath: String,
    pub filename: String,
    pub title: String,
    pub description: String,
    pub keywords: Vec<String>,
    pub categories: String,
    pub release_filename: String,
}

/// XMP fields returned when reading an image.
#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ReadResult {
    pub title: String,
    pub description: String,
    pub keywords: Vec<String>,
    pub categories: String,
    pub release_filename: String,
}

// ── XMP building ──────────────────────────────────────────────────────────

fn build_xmp(req: &SaveRequest) -> Result<Bytes, quick_xml::Error> {
    let mut w = Writer::new_with_indent(Cursor::new(Vec::<u8>::new()), b' ', 2);

    let mut xmpmeta = BytesStart::new("x:xmpmeta");
    xmpmeta.push_attribute(("xmlns:x", "adobe:ns:meta/"));
    w.write_event(Event::Start(xmpmeta))?;

    let mut rdf = BytesStart::new("rdf:RDF");
    rdf.push_attribute(("xmlns:rdf", "http://www.w3.org/1999/02/22-rdf-syntax-ns#"));
    w.write_event(Event::Start(rdf))?;

    let mut desc = BytesStart::new("rdf:Description");
    desc.push_attribute(("rdf:about", ""));
    desc.push_attribute(("xmlns:dc", "http://purl.org/dc/elements/1.1/"));
    desc.push_attribute(("xmlns:photoshop", "http://ns.adobe.com/photoshop/1.0/"));
    w.write_event(Event::Start(desc))?;

    if !req.title.is_empty() {
        w.write_event(Event::Start(BytesStart::new("dc:title")))?;
        w.write_event(Event::Start(BytesStart::new("rdf:Alt")))?;
        let mut li = BytesStart::new("rdf:li");
        li.push_attribute(("xml:lang", "x-default"));
        w.write_event(Event::Start(li))?;
        w.write_event(Event::Text(BytesText::new(&req.title)))?;
        w.write_event(Event::End(BytesEnd::new("rdf:li")))?;
        w.write_event(Event::End(BytesEnd::new("rdf:Alt")))?;
        w.write_event(Event::End(BytesEnd::new("dc:title")))?;
    }

    if !req.description.is_empty() {
        w.write_event(Event::Start(BytesStart::new("dc:description")))?;
        w.write_event(Event::Start(BytesStart::new("rdf:Alt")))?;
        let mut li = BytesStart::new("rdf:li");
        li.push_attribute(("xml:lang", "x-default"));
        w.write_event(Event::Start(li))?;
        w.write_event(Event::Text(BytesText::new(&req.description)))?;
        w.write_event(Event::End(BytesEnd::new("rdf:li")))?;
        w.write_event(Event::End(BytesEnd::new("rdf:Alt")))?;
        w.write_event(Event::End(BytesEnd::new("dc:description")))?;
    }

    if !req.keywords.is_empty() {
        w.write_event(Event::Start(BytesStart::new("dc:subject")))?;
        w.write_event(Event::Start(BytesStart::new("rdf:Bag")))?;
        for kw in &req.keywords {
            w.write_event(Event::Start(BytesStart::new("rdf:li")))?;
            w.write_event(Event::Text(BytesText::new(kw)))?;
            w.write_event(Event::End(BytesEnd::new("rdf:li")))?;
        }
        w.write_event(Event::End(BytesEnd::new("rdf:Bag")))?;
        w.write_event(Event::End(BytesEnd::new("dc:subject")))?;
    }

    if !req.categories.is_empty() {
        w.write_event(Event::Start(BytesStart::new("photoshop:Category")))?;
        w.write_event(Event::Text(BytesText::new(&req.categories)))?;
        w.write_event(Event::End(BytesEnd::new("photoshop:Category")))?;
    }

    w.write_event(Event::End(BytesEnd::new("rdf:Description")))?;
    w.write_event(Event::End(BytesEnd::new("rdf:RDF")))?;
    w.write_event(Event::End(BytesEnd::new("x:xmpmeta")))?;

    let xml_body = w.into_inner().into_inner();

    let mut packet = Vec::new();
    packet.extend_from_slice(b"<?xpacket begin='\xef\xbb\xbf' id='W5M0MpCehiHzreSzNTczkc9d'?>\n");
    packet.extend_from_slice(&xml_body);
    packet.extend_from_slice(b"\n<?xpacket end='w'?>");
    Ok(Bytes::from(packet))
}

// ── XMP parsing ───────────────────────────────────────────────────────────

/// Extract XMP fields from a raw XMP packet (UTF-8 XML bytes).
fn parse_xmp(xmp_bytes: &[u8]) -> ReadResult {
    #[derive(Clone, Copy, PartialEq)]
    enum Ctx {
        None,
        Title,
        Desc,
        Subject,
        Category,
    }

    let mut reader = Reader::from_reader(xmp_bytes);
    reader.config_mut().trim_text(true);

    let mut result = ReadResult::default();
    let mut ctx = Ctx::None;
    let mut buf = Vec::new();

    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => match e.local_name().as_ref() {
                b"title" => ctx = Ctx::Title,
                b"description" => ctx = Ctx::Desc,
                b"subject" => ctx = Ctx::Subject,
                b"Category" => ctx = Ctx::Category,
                _ => {}
            },
            Ok(Event::End(ref e)) => match e.local_name().as_ref() {
                b"title" | b"description" | b"subject" | b"Category" => ctx = Ctx::None,
                _ => {}
            },
            Ok(Event::Text(e)) => {
                if let Ok(raw) = std::str::from_utf8(e.as_ref()) {
                    let text = raw.trim().to_string();
                    if !text.is_empty() {
                        match ctx {
                            Ctx::Title if result.title.is_empty() => result.title = text,
                            Ctx::Desc if result.description.is_empty() => result.description = text,
                            Ctx::Subject => result.keywords.push(text),
                            Ctx::Category => result.categories = text,
                            _ => {}
                        }
                    }
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }

    result
}

// ── Per-format XMP read/write ──────────────────────────────────────────────

const JPEG_XMP_HEADER: &[u8] = b"http://ns.adobe.com/xap/1.0/\0";

fn get_jpeg_xmp(jpeg: &Jpeg) -> Option<Bytes> {
    jpeg.segments()
        .iter()
        .find(|seg| seg.marker() == markers::APP1 && seg.contents().starts_with(JPEG_XMP_HEADER))
        .map(|seg| seg.contents().slice(JPEG_XMP_HEADER.len()..))
}

fn set_jpeg_xmp(jpeg: &mut Jpeg, xmp: Bytes) {
    jpeg.segments_mut().retain(|seg| {
        !(seg.marker() == markers::APP1 && seg.contents().starts_with(JPEG_XMP_HEADER))
    });
    let mut contents = BytesMut::with_capacity(JPEG_XMP_HEADER.len() + xmp.len());
    contents.extend_from_slice(JPEG_XMP_HEADER);
    contents.extend_from_slice(&xmp);
    let segment = JpegSegment::new_with_contents(markers::APP1, contents.freeze());
    jpeg.segments_mut().insert(0, segment);
}

const PNG_ITXT: [u8; 4] = [b'i', b'T', b'X', b't'];
const PNG_XMP_KEYWORD: &[u8] = b"XML:com.adobe.xmp";
// iTXt header size: keyword + \0 + flags(2) + lang\0 + trans\0 = keyword_len + 5
const PNG_XMP_HEADER_LEN: usize = PNG_XMP_KEYWORD.len() + 5;

fn get_png_xmp(png: &Png) -> Option<Bytes> {
    png.chunk_by_type(PNG_ITXT)
        .filter(|chunk| chunk.contents().starts_with(PNG_XMP_KEYWORD))
        .map(|chunk| chunk.contents().slice(PNG_XMP_HEADER_LEN..))
}

fn set_png_xmp(png: &mut Png, xmp: Bytes) {
    // Remove only the XMP iTXt chunk; other iTXt chunks (copyright, comments, etc.) are preserved.
    png.chunks_mut().retain(|chunk| {
        !(chunk.kind() == PNG_ITXT && chunk.contents().starts_with(PNG_XMP_KEYWORD))
    });
    let mut contents = BytesMut::with_capacity(PNG_XMP_HEADER_LEN + xmp.len());
    contents.extend_from_slice(PNG_XMP_KEYWORD);
    contents.extend_from_slice(&[0x00, 0x00, 0x00, 0x00, 0x00]);
    contents.extend_from_slice(&xmp);
    let chunk = PngChunk::new(PNG_ITXT, contents.freeze());
    let pos = png.chunks().len().saturating_sub(1);
    png.chunks_mut().insert(pos, chunk);
}

fn get_webp_xmp(webp: &WebP) -> Option<Bytes> {
    webp.chunk_by_id(CHUNK_XMP)
        .and_then(|chunk| chunk.content().data().cloned())
}

fn set_webp_xmp(webp: &mut WebP, xmp: Bytes) {
    webp.remove_chunks_by_id(CHUNK_XMP);
    let chunk = img_parts::riff::RiffChunk::new(CHUNK_XMP, RiffContent::Data(xmp));
    webp.chunks_mut().push(chunk);
}

// ── Commands ──────────────────────────────────────────────────────────────

/// Read XMP metadata fields from an image file.
/// Returns default (empty) values if the file has no XMP.
#[tauri::command]
fn read_metadata(path: String) -> Result<ReadResult, String> {
    let p = std::path::Path::new(&path);
    info!("read_metadata: {}", p.display());

    let raw = std::fs::read(p).map_err(|e| e.to_string())?;
    let image = DynImage::from_bytes(Bytes::from(raw))
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Unsupported format: {}", p.display()))?;

    let xmp = match &image {
        DynImage::Jpeg(jpeg) => get_jpeg_xmp(jpeg),
        DynImage::Png(png) => get_png_xmp(png),
        DynImage::WebP(webp) => get_webp_xmp(webp),
    };

    let result = xmp.as_deref().map(parse_xmp).unwrap_or_default();
    debug!("read_metadata: title={:?} keywords={}", result.title, result.keywords.len());
    Ok(result)
}

/// Write XMP metadata into the image file.
/// If `filename` (stem) differs from the current stem, the file is renamed.
/// Returns the final file path (new path if renamed, original path otherwise).
#[tauri::command]
fn save_metadata(metadata: SaveRequest) -> Result<String, String> {
    let orig_path = std::path::Path::new(&metadata.filepath);
    info!("save_metadata: {}", orig_path.display());

    // ── Read and mutate image ──
    let raw = std::fs::read(orig_path).map_err(|e| e.to_string())?;
    let mut image = DynImage::from_bytes(Bytes::from(raw))
        .map_err(|e| e.to_string())?
        .ok_or_else(|| {
            let msg = format!("Unsupported format: {}", orig_path.display());
            error!("{msg}");
            msg
        })?;

    let xmp = build_xmp(&metadata).map_err(|e| e.to_string())?;
    debug!("XMP packet size: {} bytes", xmp.len());

    match &mut image {
        DynImage::Jpeg(jpeg) => set_jpeg_xmp(jpeg, xmp),
        DynImage::Png(png) => set_png_xmp(png, xmp),
        DynImage::WebP(webp) => set_webp_xmp(webp, xmp),
    }

    // ── Determine target path (rename if stem changed) ──
    let orig_stem = orig_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("");

    // Strip any extension the user may have accidentally typed
    let new_stem = {
        let s = metadata.filename.trim();
        std::path::Path::new(s)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(s)
            .to_string()
    };

    let final_path = if !new_stem.is_empty() && new_stem != orig_stem {
        let ext = orig_path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let new_name = if ext.is_empty() {
            new_stem.clone()
        } else {
            format!("{}.{}", new_stem, ext)
        };
        orig_path.parent().unwrap_or(orig_path).join(&new_name)
    } else {
        orig_path.to_path_buf()
    };

    // ── Write image ──
    let mut buf = Cursor::new(Vec::new());
    image.encoder().write_to(&mut buf).map_err(|e| e.to_string())?;
    let image_bytes = buf.into_inner();

    if final_path != orig_path {
        // create_new(true) → O_CREAT|O_EXCL: atomic check-and-create, eliminates TOCTOU.
        use std::io::Write as _;
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&final_path)
            .and_then(|mut f| f.write_all(&image_bytes))
            .map_err(|e| {
                let msg = if e.kind() == std::io::ErrorKind::AlreadyExists {
                    format!(
                        "File already exists: {}",
                        final_path.file_name().unwrap_or_default().to_string_lossy()
                    )
                } else {
                    e.to_string()
                };
                error!("Failed to write {}: {msg}", final_path.display());
                msg
            })?;
        if let Err(e) = std::fs::remove_file(orig_path) {
            error!("Failed to delete original {}: {e}", orig_path.display());
        }
        info!("renamed: {} → {}", orig_path.display(), final_path.display());
    } else {
        std::fs::write(&final_path, &image_bytes).map_err(|e| {
            let msg = e.to_string();
            error!("Failed to write {}: {msg}", final_path.display());
            msg
        })?;
    }

    info!("save_metadata: done → {}", final_path.display());
    Ok(final_path.to_string_lossy().to_string())
}

// ── File watcher state ────────────────────────────────────────────────────

pub struct WatcherState(pub Mutex<Option<RecommendedWatcher>>);

// ── File tree ─────────────────────────────────────────────────────────────

#[derive(Serialize, Clone)]
pub struct FileNode {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub children: Vec<FileNode>,
}

/// Re-scan a folder path that was previously opened (no dialog).
/// Called by the frontend after a `folder-changed` event.
#[tauri::command]
fn scan_folder(path: String) -> Result<FileNode, String> {
    let p = std::path::Path::new(&path);
    scan_dir(p).map_err(|e| e.to_string())
}

/// Start watching a folder for changes and emit `folder-changed` events.
fn start_watching(app: &tauri::AppHandle, path: &std::path::Path) {
    use tauri::Manager;
    let app_clone = app.clone();
    let watch_path = path.to_string_lossy().to_string();

    match notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        if res.is_ok() {
            use tauri::Emitter;
            app_clone.emit("folder-changed", &watch_path).ok();
        }
    }) {
        Ok(mut watcher) => {
            if let Err(e) = watcher.watch(path, RecursiveMode::Recursive) {
                error!("Failed to watch folder: {e}");
            } else {
                info!("Watching: {}", path.display());
                let state = app.state::<WatcherState>();
                // Replacing the previous watcher implicitly drops it, which stops watching the old folder.
                *state.0.lock().unwrap_or_else(|e| e.into_inner()) = Some(watcher);
            }
        }
        Err(e) => error!("Failed to create watcher: {e}"),
    }
}

/// Open a folder by path without a dialog (used to restore last folder on startup).
#[tauri::command]
async fn open_folder_path(app: tauri::AppHandle, path: String) -> Result<FileNode, String> {
    let path = std::path::PathBuf::from(&path);
    if !path.is_dir() {
        return Err(format!("Not a directory: {}", path.display()));
    }
    info!("open_folder_path: {}", path.display());
    let node = scan_dir(&path).map_err(|e| {
        let msg = e.to_string();
        error!("scan_dir failed: {msg}");
        msg
    })?;
    start_watching(&app, &path);
    Ok(node)
}

#[tauri::command]
async fn open_folder(app: tauri::AppHandle) -> Result<Option<FileNode>, String> {
    use tauri_plugin_dialog::DialogExt;

    // Non-blocking: pick_folder fires the callback from a native dialog thread.
    // blocking_pick_folder() would stall the main thread → "Not Responding" on Windows.
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog().file().pick_folder(move |result| {
        let _ = tx.send(result);
    });
    let Some(folder) = rx.await.map_err(|e| e.to_string())? else {
        info!("open_folder: cancelled");
        return Ok(None);
    };

    let path = folder.into_path().map_err(|e| e.to_string())?;
    info!("open_folder: {}", path.display());
    let node = scan_dir(&path).map_err(|e| {
        let msg = e.to_string();
        error!("scan_dir failed: {msg}");
        msg
    })?;
    start_watching(&app, &path);
    Ok(Some(node))
}

fn scan_dir(path: &std::path::Path) -> std::io::Result<FileNode> {
    let name = path
        .file_name()
        .unwrap_or(path.as_os_str())
        .to_string_lossy()
        .to_string();

    let mut children = Vec::new();

    if path.is_dir() {
        let mut entries: Vec<_> = std::fs::read_dir(path)?
            .filter_map(|e| match e {
                Ok(entry) => Some(entry),
                Err(err) => {
                    warn!("scan_dir: skipping entry in {}: {err}", path.display());
                    None
                }
            })
            .collect();

        entries.sort_by(|a, b| {
            let a_dir = a.path().is_dir();
            let b_dir = b.path().is_dir();
            b_dir.cmp(&a_dir).then_with(|| a.file_name().cmp(&b.file_name()))
        });

        for entry in entries {
            let child = entry.path();
            if child.is_dir() || is_supported_image(&child) {
                match scan_dir(&child) {
                    Ok(node) => children.push(node),
                    Err(err) => warn!("scan_dir: skipping {}: {err}", child.display()),
                }
            }
        }
    }

    Ok(FileNode {
        name,
        path: path.to_string_lossy().to_string(),
        is_dir: path.is_dir(),
        children,
    })
}

fn is_supported_image(path: &std::path::Path) -> bool {
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .as_deref(),
        Some("jpg" | "jpeg" | "png" | "webp")
    )
}

// ── App entry ─────────────────────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(WatcherState(Mutex::new(None)))
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(if cfg!(debug_assertions) {
                    log::LevelFilter::Debug
                } else {
                    log::LevelFilter::Info
                })
                .build(),
        )
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![read_metadata, save_metadata, open_folder, open_folder_path, scan_folder])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
