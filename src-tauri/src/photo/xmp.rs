//! XMP build/parse for the four managed stock fields: `dc:title`, `dc:description`,
//! `dc:subject` (keywords), and `photoshop:Category`/`Headline`.
//!
//! Why this is hand-rolled instead of delegated to the fork: `little_exif`'s `XmpData`
//! only stores the raw packet (`from_raw` / `as_bytes`) and exposes structured access
//! for the `exif:` namespace alone (camera tags via `get_exif_tags` / `set_exif_tags`).
//! It has no API for the Dublin Core / Photoshop properties this app edits, so those
//! elements are serialized and parsed here with `quick-xml` (the same crate the fork
//! uses internally). The fork still owns the XMP container, the EXIF/IPTC blocks, and
//! all streaming file I/O.

use std::fs::File;
use std::io::{BufReader, Cursor, Read, Seek, SeekFrom};
use std::path::Path;

use log::warn;
use quick_xml::events::{BytesEnd, BytesStart, BytesText, Event};
use quick_xml::Reader;
use quick_xml::Writer;

use super::Metadata;

/// Extracts `(title, headline, description, keywords, category)` from a raw XMP packet.
pub(crate) fn parse_xmp_fields(
    xmp_bytes: &[u8],
) -> (Option<String>, Option<String>, Option<String>, Vec<String>, Option<String>) {
    #[derive(Clone, Copy, PartialEq)]
    enum Ctx {
        None,
        Title,
        Desc,
        Subject,
        Headline,
        Category,
    }

    let mut reader = Reader::from_reader(xmp_bytes);
    reader.config_mut().trim_text(true);

    let mut title: Option<String> = None;
    let mut headline: Option<String> = None;
    let mut desc: Option<String> = None;
    let mut keywords: Vec<String> = Vec::new();
    let mut category: Option<String> = None;

    let mut ctx = Ctx::None;
    let mut text_buf = String::new();
    let mut buf = Vec::new();

    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => match e.local_name().as_ref() {
                b"title" => ctx = Ctx::Title,
                b"description" => ctx = Ctx::Desc,
                b"subject" => ctx = Ctx::Subject,
                b"Headline" => {
                    ctx = Ctx::Headline;
                    text_buf.clear();
                }
                b"Category" => {
                    ctx = Ctx::Category;
                    text_buf.clear();
                }
                b"li" if ctx != Ctx::None => text_buf.clear(),
                _ => {}
            },
            Ok(Event::End(ref e)) => match e.local_name().as_ref() {
                b"li" if ctx != Ctx::None => {
                    let text = text_buf.trim().to_string();
                    if !text.is_empty() {
                        match ctx {
                            Ctx::Title if title.is_none() => title = Some(text),
                            Ctx::Desc if desc.is_none() => desc = Some(text),
                            Ctx::Subject => {
                                if !keywords.contains(&text) {
                                    keywords.push(text);
                                }
                            }
                            _ => {}
                        }
                    }
                    text_buf.clear();
                }
                b"Headline" => {
                    let text = text_buf.trim().to_string();
                    if !text.is_empty() && headline.is_none() {
                        headline = Some(text);
                    }
                    text_buf.clear();
                    ctx = Ctx::None;
                }
                b"Category" => {
                    let text = text_buf.trim().to_string();
                    if !text.is_empty() && category.is_none() {
                        category = Some(text);
                    }
                    text_buf.clear();
                    ctx = Ctx::None;
                }
                b"title" | b"description" | b"subject" => ctx = Ctx::None,
                _ => {}
            },
            Ok(Event::Text(e)) => {
                if ctx != Ctx::None {
                    if let Ok(cow) = e.xml_content() {
                        text_buf.push_str(&cow);
                    }
                }
            }
            Ok(Event::GeneralRef(e)) => {
                if ctx != Ctx::None {
                    let name = e.into_inner();
                    let ch = match name.as_ref() {
                        b"amp" => "&",
                        b"lt" => "<",
                        b"gt" => ">",
                        b"apos" => "'",
                        b"quot" => "\"",
                        _ => "",
                    };
                    text_buf.push_str(ch);
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                warn!("parse_xmp_fields: XML parse error: {e}");
                break;
            }
            _ => {}
        }
    }

    (title, headline, desc, keywords, category)
}

/// Builds a complete XMP packet (xpacket-wrapped) for the managed fields.
pub(crate) fn build_xmp_packet(m: &Metadata) -> Vec<u8> {
    let body = build_xmp_body(m).unwrap_or_else(|e| {
        warn!("build_xmp_packet: failed to build XMP body: {e}");
        Vec::new()
    });
    let mut packet = Vec::new();
    packet.extend_from_slice(b"<?xpacket begin='\xef\xbb\xbf' id='W5M0MpCehiHzreSzNTczkc9d'?>\n");
    packet.extend_from_slice(&body);
    packet.extend_from_slice(b"\n<?xpacket end='w'?>");
    packet
}

fn build_xmp_body(m: &Metadata) -> Result<Vec<u8>, quick_xml::Error> {
    let mut w = Writer::new_with_indent(Cursor::new(Vec::<u8>::new()), b' ', 2);

    let mut xmpmeta = BytesStart::new("x:xmpmeta");
    xmpmeta.push_attribute(("xmlns:x", "adobe:ns:meta/"));
    w.write_event(Event::Start(xmpmeta))?;

    let mut rdf = BytesStart::new("rdf:RDF");
    rdf.push_attribute(("xmlns:rdf", "http://www.w3.org/1999/02/22-rdf-syntax-ns#"));
    w.write_event(Event::Start(rdf))?;

    let mut desc_el = BytesStart::new("rdf:Description");
    desc_el.push_attribute(("rdf:about", ""));
    desc_el.push_attribute(("xmlns:dc", "http://purl.org/dc/elements/1.1/"));
    desc_el.push_attribute(("xmlns:photoshop", "http://ns.adobe.com/photoshop/1.0/"));
    w.write_event(Event::Start(desc_el))?;

    if !m.title.is_empty() {
        w.write_event(Event::Start(BytesStart::new("dc:title")))?;
        w.write_event(Event::Start(BytesStart::new("rdf:Alt")))?;
        let mut li = BytesStart::new("rdf:li");
        li.push_attribute(("xml:lang", "x-default"));
        w.write_event(Event::Start(li))?;
        w.write_event(Event::Text(BytesText::new(&m.title)))?;
        w.write_event(Event::End(BytesEnd::new("rdf:li")))?;
        w.write_event(Event::End(BytesEnd::new("rdf:Alt")))?;
        w.write_event(Event::End(BytesEnd::new("dc:title")))?;
    }

    if !m.description.is_empty() {
        w.write_event(Event::Start(BytesStart::new("dc:description")))?;
        w.write_event(Event::Start(BytesStart::new("rdf:Alt")))?;
        let mut li = BytesStart::new("rdf:li");
        li.push_attribute(("xml:lang", "x-default"));
        w.write_event(Event::Start(li))?;
        w.write_event(Event::Text(BytesText::new(&m.description)))?;
        w.write_event(Event::End(BytesEnd::new("rdf:li")))?;
        w.write_event(Event::End(BytesEnd::new("rdf:Alt")))?;
        w.write_event(Event::End(BytesEnd::new("dc:description")))?;
    }

    if !m.keywords.is_empty() {
        w.write_event(Event::Start(BytesStart::new("dc:subject")))?;
        w.write_event(Event::Start(BytesStart::new("rdf:Bag")))?;
        for kw in &m.keywords {
            w.write_event(Event::Start(BytesStart::new("rdf:li")))?;
            w.write_event(Event::Text(BytesText::new(kw)))?;
            w.write_event(Event::End(BytesEnd::new("rdf:li")))?;
        }
        w.write_event(Event::End(BytesEnd::new("rdf:Bag")))?;
        w.write_event(Event::End(BytesEnd::new("dc:subject")))?;
    }

    if !m.title.is_empty() {
        w.write_event(Event::Start(BytesStart::new("photoshop:Headline")))?;
        w.write_event(Event::Text(BytesText::new(&m.title)))?;
        w.write_event(Event::End(BytesEnd::new("photoshop:Headline")))?;
    }

    if !m.category.is_empty() {
        w.write_event(Event::Start(BytesStart::new("photoshop:Category")))?;
        w.write_event(Event::Text(BytesText::new(&m.category)))?;
        w.write_event(Event::End(BytesEnd::new("photoshop:Category")))?;
    }

    w.write_event(Event::End(BytesEnd::new("rdf:Description")))?;
    w.write_event(Event::End(BytesEnd::new("rdf:RDF")))?;
    w.write_event(Event::End(BytesEnd::new("x:xmpmeta")))?;

    Ok(w.into_inner().into_inner())
}

// ── Streaming XMP scanners (Seek + Read; tolerant, EXIF/format-agnostic) ──────
//
// These read only the XMP packet, seeking past image data. Unlike `little_exif`'s
// all-or-nothing `new_from_path`, they succeed on XMP-only files that carry no EXIF
// (PNG) or use a non-VP8X WebP container — which the app must still read.

const JPEG_XMP_HEADER: &[u8] = b"http://ns.adobe.com/xap/1.0/\0";
const PNG_XMP_KEYWORD: &[u8] = b"XML:com.adobe.xmp";
// iTXt header size: keyword + \0 + flags(2) + lang\0 + trans\0 = keyword_len + 5
const PNG_XMP_HEADER_LEN: usize = PNG_XMP_KEYWORD.len() + 5;

/// Reads the raw XMP packet from `path` via the format-specific streaming scanner.
/// Returns `None` if the file has no XMP (or cannot be opened/scanned).
pub(crate) fn read_xmp_from_path(path: &Path) -> Option<Vec<u8>> {
    let ext = path.extension().and_then(|e| e.to_str()).map(|e| e.to_lowercase());
    let mut reader = match File::open(path) {
        Ok(f) => BufReader::new(f),
        Err(e) => {
            warn!("read_xmp_from_path: open failed for {}: {e}", path.display());
            return None;
        }
    };
    let scanned = match ext.as_deref() {
        Some("jpg") | Some("jpeg") => read_jpeg_xmp_fast(&mut reader),
        Some("png") => read_png_xmp_fast(&mut reader),
        Some("webp") => read_webp_xmp_fast(&mut reader),
        _ => Ok(None),
    };
    match scanned {
        Ok(opt) => opt,
        Err(e) => {
            warn!("read_xmp_from_path: scan failed for {}: {e}", path.display());
            None
        }
    }
}

fn read_jpeg_xmp_fast<R: Read + Seek>(r: &mut R) -> std::io::Result<Option<Vec<u8>>> {
    let mut b2 = [0u8; 2];

    r.read_exact(&mut b2)?;
    if b2 != [0xFF, 0xD8] {
        return Ok(None); // not JPEG
    }

    loop {
        r.read_exact(&mut b2)?;
        if b2[0] != 0xFF {
            return Ok(None); // corrupt
        }
        // Consume fill bytes (0xFF padding before the marker code)
        let mut m = b2[1];
        while m == 0xFF {
            r.read_exact(&mut b2[1..2])?;
            m = b2[1];
        }

        // Standalone markers without a length field
        if m == 0xD8 || m == 0xD9 || (0xD0..=0xD7).contains(&m) {
            if m == 0xD9 {
                break; // EOI
            }
            continue;
        }
        // SOS — image data starts; no more metadata segments follow
        if m == 0xDA {
            break;
        }

        r.read_exact(&mut b2)?;
        let data_len = (u16::from_be_bytes(b2) as usize).saturating_sub(2);

        if m == 0xE1 {
            // APP1: may be XMP or EXIF — always small, read fully
            let mut data = vec![0u8; data_len];
            r.read_exact(&mut data)?;
            if data.starts_with(JPEG_XMP_HEADER) {
                return Ok(Some(data[JPEG_XMP_HEADER.len()..].to_vec()));
            }
        } else {
            r.seek(SeekFrom::Current(data_len as i64))?;
        }
    }

    Ok(None)
}

fn read_png_xmp_fast<R: Read + Seek>(r: &mut R) -> std::io::Result<Option<Vec<u8>>> {
    let mut hdr = [0u8; 8];

    r.read_exact(&mut hdr)?;
    if hdr != [137, 80, 78, 71, 13, 10, 26, 10] {
        return Ok(None); // not PNG
    }

    loop {
        match r.read_exact(&mut hdr) {
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(e),
            Ok(()) => {}
        }
        let length = u32::from_be_bytes([hdr[0], hdr[1], hdr[2], hdr[3]]) as usize;
        let kind = &hdr[4..8];

        if kind == b"IEND" {
            break; // only stop at end-of-file; iTXt XMP may appear after IDAT
        }

        if kind == b"iTXt" {
            let mut data = vec![0u8; length];
            r.read_exact(&mut data)?;
            r.seek(SeekFrom::Current(4))?; // skip CRC
            if data.starts_with(PNG_XMP_KEYWORD) {
                return Ok(Some(data[PNG_XMP_HEADER_LEN..].to_vec()));
            }
        } else {
            r.seek(SeekFrom::Current(length as i64 + 4))?; // data + CRC
        }
    }

    Ok(None)
}

fn read_webp_xmp_fast<R: Read + Seek>(r: &mut R) -> std::io::Result<Option<Vec<u8>>> {
    let mut riff = [0u8; 12];

    r.read_exact(&mut riff)?;
    if &riff[0..4] != b"RIFF" || &riff[8..12] != b"WEBP" {
        return Ok(None); // not WebP
    }

    let mut chdr = [0u8; 8]; // FourCC (4) + size (4, little-endian)
    loop {
        match r.read_exact(&mut chdr) {
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(e),
            Ok(()) => {}
        }
        let size = u32::from_le_bytes([chdr[4], chdr[5], chdr[6], chdr[7]]) as usize;

        if &chdr[0..4] == b"XMP " {
            let mut data = vec![0u8; size];
            r.read_exact(&mut data)?;
            return Ok(Some(data));
        }
        // RIFF chunks are padded to even size
        r.seek(SeekFrom::Current((size + (size & 1)) as i64))?;
    }

    Ok(None)
}
