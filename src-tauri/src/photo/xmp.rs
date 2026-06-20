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

use std::io::Cursor;

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
