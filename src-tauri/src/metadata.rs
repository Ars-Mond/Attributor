use std::collections::HashSet;
use std::fs;
use std::io::Cursor;
use std::path::Path;

use log::{debug, info, warn};
use quick_xml::events::{BytesEnd, BytesStart, BytesText, Event};
use quick_xml::Reader;
use quick_xml::Writer;
use serde::Serialize;

use little_exif::exif_tag::ExifTag;
use little_exif::exif_tag_format::Utf16String;
use little_exif::filetype::{get_file_type, FileExtension};
use little_exif::iptc::IptcData;
use little_exif::metadata::Metadata as LE_Metadata;
use little_exif::xmp::XmpData;

// IPTC record 2 dataset numbers (decimal)
const R2: u8 = 2;
const DS_OBJECT_NAME: u8 = 5;
const DS_KEYWORDS: u8 = 25;
const DS_HEADLINE: u8 = 105;
const DS_CAPTION_ABSTRACT: u8 = 120;

#[derive(Serialize, Default, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Metadata {
    pub title: String,
    pub description: String,
    pub keywords: Vec<String>,
    pub category: String,
}

pub fn read_metadata(filepath: String) -> Result<Metadata, String> {
    let path = Path::new(&filepath);
    info!("read_metadata: {}", path.display());

    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    let file_type = get_file_type(path).map_err(|e| e.to_string())?;
    let is_jpeg = matches!(file_type, FileExtension::JPEG);

    let le = LE_Metadata::new_from_vec(&bytes, file_type).unwrap_or_else(|e| {
        warn!("read_metadata: failed to parse metadata from {}: {e}", path.display());
        LE_Metadata::new()
    });

    // EXIF
    let exif_image_desc = tag_string(&le, 0x010e);
    let exif_xp_title   = tag_utf16(&le, 0x9c9b);
    let exif_xp_keywords = tag_utf16(&le, 0x9c9e);
    let exif_xp_subject = tag_utf16(&le, 0x9c9f);

    // IPTC (JPEG only)
    let (iptc_object_name, iptc_headline, iptc_caption, iptc_keywords) = if is_jpeg {
        if let Some(iptc) = le.get_iptc() {
            (
                iptc_string(iptc, R2, DS_OBJECT_NAME),
                iptc_string(iptc, R2, DS_HEADLINE),
                iptc_string(iptc, R2, DS_CAPTION_ABSTRACT),
                iptc_all_strings(iptc, R2, DS_KEYWORDS),
            )
        } else {
            (None, None, None, vec![])
        }
    } else {
        (None, None, None, vec![])
    };

    // XMP
    let (xmp_title, xmp_headline, xmp_desc, xmp_keywords, xmp_category) =
        if let Some(xmp) = le.get_xmp() {
            parse_xmp_fields(xmp.as_bytes())
        } else {
            (None, None, None, vec![], None)
        };

    // Merge by priority
    let title = if is_jpeg {
        iptc_object_name.or(iptc_headline).or(exif_xp_title).or(xmp_title).or(xmp_headline)
    } else {
        exif_xp_title.or(xmp_title).or(xmp_headline)
    }
    .unwrap_or_default();

    let description = if is_jpeg {
        iptc_caption.or(exif_image_desc).or(exif_xp_subject).or(xmp_desc)
    } else {
        exif_image_desc.or(exif_xp_subject).or(xmp_desc)
    }
    .unwrap_or_default();

    let keywords = if is_jpeg {
        unique_keywords([
            iptc_keywords,
            exif_xp_keywords.map(split_semicolons).unwrap_or_default(),
            xmp_keywords,
        ])
    } else {
        unique_keywords([
            exif_xp_keywords.map(split_semicolons).unwrap_or_default(),
            xmp_keywords,
        ])
    };

    let category = xmp_category.unwrap_or_default();

    debug!(
        "read_metadata: title={:?} kw_count={} category={:?}",
        title,
        keywords.len(),
        category
    );

    Ok(Metadata { title, description, keywords, category })
}

pub fn write_metadata(filepath: String, metadata: Metadata) -> Result<(), String> {
    let path = Path::new(&filepath);
    info!("write_metadata: {}", path.display());

    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    let file_type = get_file_type(path).map_err(|e| e.to_string())?;
    let is_jpeg = matches!(file_type, FileExtension::JPEG);

    let mut le = LE_Metadata::new_from_vec(&bytes, file_type).unwrap_or_else(|e| {
        warn!("write_metadata: failed to parse existing metadata, starting fresh: {e}");
        LE_Metadata::new()
    });

    // EXIF
    le.set_tag(ExifTag::ImageDescription(metadata.description.clone()));
    le.set_tag(ExifTag::XPTitle(Utf16String::from(metadata.title.as_str())));
    le.set_tag(ExifTag::XPSubject(Utf16String::from(metadata.description.as_str())));
    le.set_tag(ExifTag::XPKeywords(Utf16String::from(metadata.keywords.join(";").as_str())));

    // IPTC (JPEG only)
    if is_jpeg {
        let has_iptc_content = !metadata.title.is_empty()
            || !metadata.description.is_empty()
            || !metadata.keywords.is_empty();
        if has_iptc_content {
            le.set_iptc(build_iptc(&metadata));
        } else {
            le.clear_iptc();
        }
    }

    // XMP
    le.set_xmp(XmpData::from_raw(build_xmp_packet(&metadata)));

    // Write back
    let mut buf = bytes;
    le.write_to_vec(&mut buf, file_type).map_err(|e| e.to_string())?;
    fs::write(path, &buf).map_err(|e| e.to_string())?;

    info!("write_metadata: done → {}", path.display());
    Ok(())
}

// ── EXIF helpers ─────────────────────────────────────────────────────────────

fn tag_string(le: &LE_Metadata, hex: u16) -> Option<String> {
    le.get_tag_by_hex(hex, None).next().and_then(|tag| {
        match tag {
            ExifTag::ImageDescription(s) => non_empty(s.clone()),
            _ => None,
        }
    })
}

fn tag_utf16(le: &LE_Metadata, hex: u16) -> Option<String> {
    le.get_tag_by_hex(hex, None).next().and_then(|tag| {
        let s = match tag {
            ExifTag::XPTitle(v)    => &v.0,
            ExifTag::XPKeywords(v) => &v.0,
            ExifTag::XPSubject(v)  => &v.0,
            _ => return None,
        };
        non_empty(s.clone())
    })
}

// ── IPTC helpers ──────────────────────────────────────────────────────────────

fn iptc_string(iptc: &IptcData, record: u8, dataset: u8) -> Option<String> {
    iptc.get_fields(record, dataset)
        .first()
        .and_then(|f| String::from_utf8(f.data.clone()).ok())
        .and_then(non_empty)
}

fn iptc_all_strings(iptc: &IptcData, record: u8, dataset: u8) -> Vec<String> {
    iptc.get_fields(record, dataset)
        .into_iter()
        .filter_map(|f| String::from_utf8(f.data.clone()).ok())
        .filter_map(non_empty)
        .collect()
}

// ── XMP parsing ───────────────────────────────────────────────────────────────

fn parse_xmp_fields(
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
                        b"amp"  => "&",
                        b"lt"   => "<",
                        b"gt"   => ">",
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

// ── IPTC building ────────────────────────────────────────────────────────────

fn build_iptc(m: &Metadata) -> IptcData {
    let mut iptc = IptcData::new();
    if !m.title.is_empty() {
        iptc.set_field(R2, DS_OBJECT_NAME, m.title.as_bytes().to_vec());
        iptc.set_field(R2, DS_HEADLINE, m.title.as_bytes().to_vec());
    }
    if !m.description.is_empty() {
        iptc.set_field(R2, DS_CAPTION_ABSTRACT, m.description.as_bytes().to_vec());
    }
    for kw in &m.keywords {
        iptc.add_field(R2, DS_KEYWORDS, kw.as_bytes().to_vec());
    }
    iptc
}

// ── XMP building ──────────────────────────────────────────────────────────────

fn build_xmp_packet(m: &Metadata) -> Vec<u8> {
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

// ── Utility ───────────────────────────────────────────────────────────────────

fn unique_keywords<I>(sources: I) -> Vec<String>
where
    I: IntoIterator<Item = Vec<String>>,
{
    let mut seen = HashSet::new();
    let mut result = Vec::new();
    for source in sources {
        for kw in source {
            let kw = kw.trim().to_string();
            if !kw.is_empty() && seen.insert(kw.clone()) {
                result.push(kw);
            }
        }
    }
    result
}

fn split_semicolons(s: String) -> Vec<String> {
    s.split(';')
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .collect()
}

fn non_empty(s: String) -> Option<String> {
    let trimmed = s.trim_matches(|c: char| c.is_whitespace() || c == '\0').to_string();
    if trimmed.is_empty() { None } else { Some(trimmed) }
}
