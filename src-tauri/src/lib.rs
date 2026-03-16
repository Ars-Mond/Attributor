use bytes::{Bytes, BytesMut};
use img_parts::jpeg::{markers, Jpeg, JpegSegment};
use log::{debug, error, info};
use img_parts::png::{Png, PngChunk};
use img_parts::riff::RiffContent;
use img_parts::webp::{WebP, CHUNK_XMP};
use img_parts::DynImage;
use quick_xml::events::{BytesEnd, BytesStart, BytesText, Event};
use quick_xml::Writer;
use serde::{Deserialize, Serialize};
use std::io::Cursor;

// ── Metadata ──────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct Metadata {
    pub filename: String,
    pub title: String,
    pub description: String,
    pub keywords: Vec<String>,
    pub categories: String,
    pub release_filename: String,
}

// ── XMP building ──────────────────────────────────────────────────────────

/// Build a complete XMP packet from metadata.
fn build_xmp(meta: &Metadata) -> Bytes {
    let mut w = Writer::new_with_indent(Cursor::new(Vec::<u8>::new()), b' ', 2);

    let mut xmpmeta = BytesStart::new("x:xmpmeta");
    xmpmeta.push_attribute(("xmlns:x", "adobe:ns:meta/"));
    w.write_event(Event::Start(xmpmeta)).unwrap();

    let mut rdf = BytesStart::new("rdf:RDF");
    rdf.push_attribute(("xmlns:rdf", "http://www.w3.org/1999/02/22-rdf-syntax-ns#"));
    w.write_event(Event::Start(rdf)).unwrap();

    let mut desc = BytesStart::new("rdf:Description");
    desc.push_attribute(("rdf:about", ""));
    desc.push_attribute(("xmlns:dc", "http://purl.org/dc/elements/1.1/"));
    desc.push_attribute(("xmlns:photoshop", "http://ns.adobe.com/photoshop/1.0/"));
    w.write_event(Event::Start(desc)).unwrap();

    if !meta.title.is_empty() {
        w.write_event(Event::Start(BytesStart::new("dc:title"))).unwrap();
        w.write_event(Event::Start(BytesStart::new("rdf:Alt"))).unwrap();
        let mut li = BytesStart::new("rdf:li");
        li.push_attribute(("xml:lang", "x-default"));
        w.write_event(Event::Start(li)).unwrap();
        w.write_event(Event::Text(BytesText::new(&meta.title))).unwrap();
        w.write_event(Event::End(BytesEnd::new("rdf:li"))).unwrap();
        w.write_event(Event::End(BytesEnd::new("rdf:Alt"))).unwrap();
        w.write_event(Event::End(BytesEnd::new("dc:title"))).unwrap();
    }

    if !meta.description.is_empty() {
        w.write_event(Event::Start(BytesStart::new("dc:description"))).unwrap();
        w.write_event(Event::Start(BytesStart::new("rdf:Alt"))).unwrap();
        let mut li = BytesStart::new("rdf:li");
        li.push_attribute(("xml:lang", "x-default"));
        w.write_event(Event::Start(li)).unwrap();
        w.write_event(Event::Text(BytesText::new(&meta.description))).unwrap();
        w.write_event(Event::End(BytesEnd::new("rdf:li"))).unwrap();
        w.write_event(Event::End(BytesEnd::new("rdf:Alt"))).unwrap();
        w.write_event(Event::End(BytesEnd::new("dc:description"))).unwrap();
    }

    if !meta.keywords.is_empty() {
        w.write_event(Event::Start(BytesStart::new("dc:subject"))).unwrap();
        w.write_event(Event::Start(BytesStart::new("rdf:Bag"))).unwrap();
        for kw in &meta.keywords {
            w.write_event(Event::Start(BytesStart::new("rdf:li"))).unwrap();
            w.write_event(Event::Text(BytesText::new(kw))).unwrap();
            w.write_event(Event::End(BytesEnd::new("rdf:li"))).unwrap();
        }
        w.write_event(Event::End(BytesEnd::new("rdf:Bag"))).unwrap();
        w.write_event(Event::End(BytesEnd::new("dc:subject"))).unwrap();
    }

    if !meta.categories.is_empty() {
        w.write_event(Event::Start(BytesStart::new("photoshop:Category"))).unwrap();
        w.write_event(Event::Text(BytesText::new(&meta.categories))).unwrap();
        w.write_event(Event::End(BytesEnd::new("photoshop:Category"))).unwrap();
    }

    w.write_event(Event::End(BytesEnd::new("rdf:Description"))).unwrap();
    w.write_event(Event::End(BytesEnd::new("rdf:RDF"))).unwrap();
    w.write_event(Event::End(BytesEnd::new("x:xmpmeta"))).unwrap();

    let xml_body = w.into_inner().into_inner();

    let mut packet = Vec::new();
    // XMP packet header — BOM flags UTF-8
    packet.extend_from_slice(b"<?xpacket begin='\xef\xbb\xbf' id='W5M0MpCehiHzreSzNTczkc9d'?>\n");
    packet.extend_from_slice(&xml_body);
    packet.extend_from_slice(b"\n<?xpacket end='w'?>");
    Bytes::from(packet)
}

// ── Per-format XMP embedding ───────────────────────────────────────────────

const JPEG_XMP_HEADER: &[u8] = b"http://ns.adobe.com/xap/1.0/\0";

/// Embed XMP into a JPEG as an APP1 segment.
fn set_jpeg_xmp(jpeg: &mut Jpeg, xmp: Bytes) {
    // Remove any pre-existing XMP APP1 segments.
    jpeg.segments_mut().retain(|seg| {
        !(seg.marker() == markers::APP1 && seg.contents().starts_with(JPEG_XMP_HEADER))
    });

    let mut contents = BytesMut::with_capacity(JPEG_XMP_HEADER.len() + xmp.len());
    contents.extend_from_slice(JPEG_XMP_HEADER);
    contents.extend_from_slice(&xmp);

    let segment = JpegSegment::new_with_contents(markers::APP1, contents.freeze());
    // Insert right after SOI (index 0 = first real segment slot).
    jpeg.segments_mut().insert(0, segment);
}

const PNG_ITXT: [u8; 4] = [b'i', b'T', b'X', b't'];
const PNG_XMP_KEYWORD: &[u8] = b"XML:com.adobe.xmp";

/// Embed XMP into a PNG as an uncompressed iTXt chunk.
fn set_png_xmp(png: &mut Png, xmp: Bytes) {
    png.remove_chunks_by_type(PNG_ITXT);

    // iTXt layout:
    //   keyword \0 compression_flag(0) compression_method(0)
    //   language_tag \0 translated_keyword \0 text
    let mut contents = BytesMut::with_capacity(PNG_XMP_KEYWORD.len() + 5 + xmp.len());
    contents.extend_from_slice(PNG_XMP_KEYWORD);
    contents.extend_from_slice(&[0x00, 0x00, 0x00, 0x00, 0x00]); // \0, no compression, no lang/trans
    contents.extend_from_slice(&xmp);

    let chunk = PngChunk::new(PNG_ITXT, contents.freeze());
    // Insert before the last chunk (IEND).
    let pos = png.chunks().len().saturating_sub(1);
    png.chunks_mut().insert(pos, chunk);
}

/// Embed XMP into a WebP as an XMP  chunk.
fn set_webp_xmp(webp: &mut WebP, xmp: Bytes) {
    webp.remove_chunks_by_id(CHUNK_XMP);

    let chunk = img_parts::riff::RiffChunk::new(CHUNK_XMP, RiffContent::Data(xmp));
    webp.chunks_mut().push(chunk);
}

// ── Command ────────────────────────────────────────────────────────────────

/// Write XMP metadata into the image file in-place.
#[tauri::command]
fn save_metadata(metadata: Metadata) -> Result<(), String> {
    let path = std::path::Path::new(&metadata.filename);
    info!("save_metadata: {}", path.display());

    let raw = std::fs::read(path).map_err(|e| e.to_string())?;
    let mut image = DynImage::from_bytes(Bytes::from(raw))
        .map_err(|e| e.to_string())?
        .ok_or_else(|| {
            let msg = format!("Unsupported format: {}", path.display());
            error!("{msg}");
            msg
        })?;

    let xmp = build_xmp(&metadata);
    debug!("XMP packet size: {} bytes", xmp.len());

    match &mut image {
        DynImage::Jpeg(jpeg) => set_jpeg_xmp(jpeg, xmp),
        DynImage::Png(png) => set_png_xmp(png, xmp),
        DynImage::WebP(webp) => set_webp_xmp(webp, xmp),
    }

    let mut buf = Cursor::new(Vec::new());
    image
        .encoder()
        .write_to(&mut buf)
        .map_err(|e| e.to_string())?;

    std::fs::write(path, buf.into_inner()).map_err(|e| {
        let msg = e.to_string();
        error!("Failed to write {}: {msg}", path.display());
        msg
    })?;

    info!("save_metadata: done");
    Ok(())
}

// ── File tree ─────────────────────────────────────────────────────────────

#[derive(Serialize, Clone)]
pub struct FileNode {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub children: Vec<FileNode>,
}

/// Open a folder picker dialog and return a file-system tree
/// containing only supported image files and subdirectories.
#[tauri::command]
fn open_folder(app: tauri::AppHandle) -> Result<Option<FileNode>, String> {
    use tauri_plugin_dialog::DialogExt;

    let Some(folder) = app.dialog().file().blocking_pick_folder() else {
        info!("open_folder: cancelled");
        return Ok(None);
    };

    let path = std::path::PathBuf::from(folder.to_string());
    info!("open_folder: {}", path.display());
    let node = scan_dir(&path).map_err(|e| {
        let msg = e.to_string();
        error!("scan_dir failed: {msg}");
        msg
    })?;
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
            .filter_map(|e| e.ok())
            .collect();

        entries.sort_by(|a, b| {
            let a_dir = a.path().is_dir();
            let b_dir = b.path().is_dir();
            b_dir.cmp(&a_dir).then_with(|| a.file_name().cmp(&b.file_name()))
        });

        for entry in entries {
            let child = entry.path();
            if child.is_dir() || is_supported_image(&child) {
                if let Ok(node) = scan_dir(&child) {
                    children.push(node);
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

/// img-parts 0.4.0 supports JPEG, PNG, and WebP only.
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
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Debug)
                .build(),
        )
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![save_metadata, open_folder])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
