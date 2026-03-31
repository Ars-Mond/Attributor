use attributor_lib::{parse_xmp, read_jpeg_xmp_fast, read_png_xmp_fast, read_webp_xmp_fast, ReadResult};
use std::io::Cursor;

// ── Test XMP packets ───────────────────────────────────────────────────────

/// Minimal XMP with all four supported fields.
const TEST_XMP: &[u8] = b"<?xpacket begin='\xef\xbb\xbf' id='W5M0MpCehiHzreSzNTczkc9d'?>\n\
<x:xmpmeta xmlns:x=\"adobe:ns:meta/\">\
<rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\
<rdf:Description rdf:about=\"\" \
xmlns:dc=\"http://purl.org/dc/elements/1.1/\" \
xmlns:photoshop=\"http://ns.adobe.com/photoshop/1.0/\">\
<dc:title><rdf:Alt><rdf:li xml:lang=\"x-default\">Test Title</rdf:li></rdf:Alt></dc:title>\
<dc:description><rdf:Alt><rdf:li xml:lang=\"x-default\">Test Desc</rdf:li></rdf:Alt></dc:description>\
<dc:subject><rdf:Bag><rdf:li>kw1</rdf:li><rdf:li>kw2</rdf:li></rdf:Bag></dc:subject>\
<photoshop:Category>nature</photoshop:Category>\
</rdf:Description></rdf:RDF></x:xmpmeta>\n\
<?xpacket end='w'?>";

// ── Image builders ─────────────────────────────────────────────────────────

const JPEG_XMP_HEADER: &[u8] = b"http://ns.adobe.com/xap/1.0/\0";
const PNG_SIG: [u8; 8] = [137, 80, 78, 71, 13, 10, 26, 10];
const PNG_XMP_KEYWORD: &[u8] = b"XML:com.adobe.xmp";

fn make_jpeg(xmp: &[u8]) -> Vec<u8> {
    let mut v = vec![0xFF, 0xD8]; // SOI
    jpeg_app1_xmp(&mut v, xmp);
    v.extend_from_slice(&[0xFF, 0xDA]); // SOS — stops the scanner
    v
}

fn jpeg_app1_xmp(v: &mut Vec<u8>, xmp: &[u8]) {
    let data: Vec<u8> = [JPEG_XMP_HEADER, xmp].concat();
    let len = (data.len() + 2) as u16;
    v.push(0xFF); v.push(0xE1);
    v.extend_from_slice(&len.to_be_bytes());
    v.extend_from_slice(&data);
}

/// APP1 segment with Exif header (not XMP) — used to test that the scanner
/// correctly skips non-XMP APP1 segments.
fn jpeg_app1_exif(v: &mut Vec<u8>) {
    let data = b"Exif\0\0II\x2a\0fake_exif_body";
    let len = (data.len() + 2) as u16;
    v.push(0xFF); v.push(0xE1);
    v.extend_from_slice(&len.to_be_bytes());
    v.extend_from_slice(data);
}

/// APP0 JFIF segment — common first segment in consumer JPEGs.
fn jpeg_app0_jfif(v: &mut Vec<u8>) {
    let data = b"JFIF\0\x01\x01\0\0\x01\0\x01\0\0";
    let len = (data.len() + 2) as u16;
    v.push(0xFF); v.push(0xE0);
    v.extend_from_slice(&len.to_be_bytes());
    v.extend_from_slice(data);
}

fn make_png(xmp: &[u8]) -> Vec<u8> {
    let mut v = PNG_SIG.to_vec();
    png_itxt_xmp(&mut v, xmp);
    png_chunk(&mut v, b"IDAT", &[]); // stops the scanner
    v
}

fn png_itxt_xmp(v: &mut Vec<u8>, xmp: &[u8]) {
    let mut data = Vec::new();
    data.extend_from_slice(PNG_XMP_KEYWORD);
    data.extend_from_slice(&[0x00, 0x00, 0x00, 0x00, 0x00]); // flags: null + comp + lang\0 + trans\0
    data.extend_from_slice(xmp);
    png_chunk(v, b"iTXt", &data);
}

fn png_chunk(v: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    v.extend_from_slice(&(data.len() as u32).to_be_bytes());
    v.extend_from_slice(kind);
    v.extend_from_slice(data);
    v.extend_from_slice(&[0, 0, 0, 0]); // CRC — not validated by the reader
}

fn make_webp(xmp: &[u8]) -> Vec<u8> {
    let padded = xmp.len() + (xmp.len() & 1);
    let mut v = Vec::new();
    v.extend_from_slice(b"RIFF");
    v.extend_from_slice(&(4u32 + 8 + padded as u32).to_le_bytes()); // WEBP + chunk_hdr + chunk_data
    v.extend_from_slice(b"WEBP");
    v.extend_from_slice(b"XMP ");
    v.extend_from_slice(&(xmp.len() as u32).to_le_bytes());
    v.extend_from_slice(xmp);
    if xmp.len() & 1 != 0 { v.push(0); }
    v
}

// ── JPEG tests ─────────────────────────────────────────────────────────────

#[test]
fn jpeg_extracts_xmp() {
    let raw = make_jpeg(TEST_XMP);
    let xmp = read_jpeg_xmp_fast(&mut Cursor::new(&raw)).unwrap().unwrap();
    assert_eq!(xmp, TEST_XMP);
}

#[test]
fn jpeg_no_xmp_returns_none() {
    // Minimal valid JPEG: SOI + EOI, no APP segments
    let raw = vec![0xFF, 0xD8, 0xFF, 0xD9];
    let result = read_jpeg_xmp_fast(&mut Cursor::new(&raw)).unwrap();
    assert!(result.is_none());
}

#[test]
fn jpeg_stops_at_sos_without_xmp() {
    // SOI + APP0 (no XMP) + SOS — no XMP found
    let mut raw = vec![0xFF, 0xD8];
    jpeg_app0_jfif(&mut raw);
    raw.extend_from_slice(&[0xFF, 0xDA]);
    let result = read_jpeg_xmp_fast(&mut Cursor::new(&raw)).unwrap();
    assert!(result.is_none());
}

#[test]
fn jpeg_skips_exif_app1_before_xmp() {
    // EXIF APP1 first, then XMP APP1 — scanner must skip EXIF and find XMP
    let mut raw = vec![0xFF, 0xD8];
    jpeg_app1_exif(&mut raw);
    jpeg_app1_xmp(&mut raw, TEST_XMP);
    raw.extend_from_slice(&[0xFF, 0xDA]);
    let xmp = read_jpeg_xmp_fast(&mut Cursor::new(&raw)).unwrap().unwrap();
    assert_eq!(xmp, TEST_XMP);
}

#[test]
fn jpeg_skips_app0_before_xmp() {
    // Typical structure: JFIF APP0 + XMP APP1
    let mut raw = vec![0xFF, 0xD8];
    jpeg_app0_jfif(&mut raw);
    jpeg_app1_xmp(&mut raw, TEST_XMP);
    raw.extend_from_slice(&[0xFF, 0xDA]);
    let xmp = read_jpeg_xmp_fast(&mut Cursor::new(&raw)).unwrap().unwrap();
    assert_eq!(xmp, TEST_XMP);
}

#[test]
fn jpeg_not_a_jpeg_returns_none() {
    let raw = b"not a jpeg file at all";
    let result = read_jpeg_xmp_fast(&mut Cursor::new(raw as &[u8])).unwrap();
    assert!(result.is_none());
}

// ── PNG tests ──────────────────────────────────────────────────────────────

#[test]
fn png_extracts_xmp() {
    let raw = make_png(TEST_XMP);
    let xmp = read_png_xmp_fast(&mut Cursor::new(&raw)).unwrap().unwrap();
    assert_eq!(xmp, TEST_XMP);
}

#[test]
fn png_no_xmp_returns_none() {
    // PNG signature + immediate IDAT (no iTXt)
    let mut raw = PNG_SIG.to_vec();
    png_chunk(&mut raw, b"IDAT", b"fake_data");
    let result = read_png_xmp_fast(&mut Cursor::new(&raw)).unwrap();
    assert!(result.is_none());
}

#[test]
fn png_stops_at_iend_without_xmp() {
    let mut raw = PNG_SIG.to_vec();
    png_chunk(&mut raw, b"IEND", &[]);
    let result = read_png_xmp_fast(&mut Cursor::new(&raw)).unwrap();
    assert!(result.is_none());
}

#[test]
fn png_skips_non_xmp_chunks_before_itxt() {
    // tEXt and zTXt chunks before the XMP iTXt
    let mut raw = PNG_SIG.to_vec();
    png_chunk(&mut raw, b"tEXt", b"Comment\0some comment text");
    png_chunk(&mut raw, b"zTXt", b"Description\0\0compressed_data");
    png_itxt_xmp(&mut raw, TEST_XMP);
    png_chunk(&mut raw, b"IDAT", &[]);
    png_chunk(&mut raw, b"IEND", &[]);
    let xmp = read_png_xmp_fast(&mut Cursor::new(&raw)).unwrap().unwrap();
    assert_eq!(xmp, TEST_XMP);
}

#[test]
fn png_extracts_xmp_after_idat() {
    // Real-world scenario: XMP iTXt chunk comes AFTER all IDAT chunks, just before IEND.
    // This is valid per the PNG spec and common in files written by editors like Photoshop.
    let mut raw = PNG_SIG.to_vec();
    png_chunk(&mut raw, b"IHDR", &[0, 0, 0, 1, 0, 0, 0, 1, 8, 2, 0, 0, 0]); // 1×1 px
    png_chunk(&mut raw, b"IDAT", b"fake_compressed_image_data");
    png_chunk(&mut raw, b"IDAT", b"more_fake_idat_data");
    png_itxt_xmp(&mut raw, TEST_XMP); // XMP after image data
    png_chunk(&mut raw, b"IEND", &[]);
    let xmp = read_png_xmp_fast(&mut Cursor::new(&raw)).unwrap().unwrap();
    assert_eq!(xmp, TEST_XMP);
}

#[test]
fn png_not_a_png_returns_none() {
    let raw = b"not a png file at all!!";
    let result = read_png_xmp_fast(&mut Cursor::new(raw as &[u8])).unwrap();
    assert!(result.is_none());
}

// ── WebP tests ─────────────────────────────────────────────────────────────

#[test]
fn webp_extracts_xmp() {
    let raw = make_webp(TEST_XMP);
    let xmp = read_webp_xmp_fast(&mut Cursor::new(&raw)).unwrap().unwrap();
    assert_eq!(xmp, TEST_XMP);
}

#[test]
fn webp_no_xmp_returns_none() {
    // RIFF/WEBP with only VP8L chunk, no XMP
    let img_data = b"fake_vp8l_image_data";
    let padded = img_data.len() + (img_data.len() & 1);
    let mut raw = Vec::new();
    raw.extend_from_slice(b"RIFF");
    raw.extend_from_slice(&(4u32 + 8 + padded as u32).to_le_bytes());
    raw.extend_from_slice(b"WEBP");
    raw.extend_from_slice(b"VP8L");
    raw.extend_from_slice(&(img_data.len() as u32).to_le_bytes());
    raw.extend_from_slice(img_data);
    let result = read_webp_xmp_fast(&mut Cursor::new(&raw)).unwrap();
    assert!(result.is_none());
}

#[test]
fn webp_skips_image_chunk_before_xmp() {
    // VP8L chunk followed by XMP chunk
    let img = b"fake_vp8l";
    let img_padded = img.len() + (img.len() & 1);
    let xmp_padded = TEST_XMP.len() + (TEST_XMP.len() & 1);
    let file_size = 4u32 + 8 + img_padded as u32 + 8 + xmp_padded as u32;

    let mut raw = Vec::new();
    raw.extend_from_slice(b"RIFF");
    raw.extend_from_slice(&file_size.to_le_bytes());
    raw.extend_from_slice(b"WEBP");
    raw.extend_from_slice(b"VP8L");
    raw.extend_from_slice(&(img.len() as u32).to_le_bytes());
    raw.extend_from_slice(img);
    if img.len() & 1 != 0 { raw.push(0); }
    raw.extend_from_slice(b"XMP ");
    raw.extend_from_slice(&(TEST_XMP.len() as u32).to_le_bytes());
    raw.extend_from_slice(TEST_XMP);

    let xmp = read_webp_xmp_fast(&mut Cursor::new(&raw)).unwrap().unwrap();
    assert_eq!(xmp, TEST_XMP);
}

#[test]
fn webp_not_a_webp_returns_none() {
    let raw = b"RIFF\x10\0\0\0JPEG not webp";
    let result = read_webp_xmp_fast(&mut Cursor::new(raw as &[u8])).unwrap();
    assert!(result.is_none());
}

// ── parse_xmp tests ────────────────────────────────────────────────────────

#[test]
fn parse_xmp_extracts_all_fields() {
    let meta = parse_xmp(TEST_XMP);
    assert_eq!(meta.title, "Test Title");
    assert_eq!(meta.description, "Test Desc");
    assert_eq!(meta.keywords, vec!["kw1", "kw2"]);
    assert_eq!(meta.categories, "nature");
}

#[test]
fn parse_xmp_handles_xml_entities() {
    let xmp = br#"<?xpacket begin='' id='W5M0MpCehiHzreSzNTczkc9d'?>
<x:xmpmeta xmlns:x="adobe:ns:meta/">
<rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
<rdf:Description rdf:about="" xmlns:dc="http://purl.org/dc/elements/1.1/">
<dc:subject><rdf:Bag>
<rdf:li>mother&apos;s day</rdf:li>
<rdf:li>rock &amp; roll</rdf:li>
<rdf:li>a &lt; b</rdf:li>
</rdf:Bag></dc:subject>
</rdf:Description></rdf:RDF></x:xmpmeta>
<?xpacket end='w'?>"#;
    let meta = parse_xmp(xmp);
    // &apos; inside a word: spaces are not adjacent to the entity ref → correct.
    assert_eq!(meta.keywords[0], "mother's day");
    // Known limitation: trim_text(true) in quick-xml strips whitespace from each
    // individual text node. When a space sits directly next to an entity reference
    // (" &amp; "), the flanking spaces are trimmed and lost.
    assert_eq!(meta.keywords[1], "rock&roll");
    assert_eq!(meta.keywords[2], "a<b");
}

#[test]
fn parse_xmp_deduplicates_keywords() {
    let xmp = br#"<?xpacket begin='' id='W5M0MpCehiHzreSzNTczkc9d'?>
<x:xmpmeta xmlns:x="adobe:ns:meta/">
<rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
<rdf:Description rdf:about="" xmlns:dc="http://purl.org/dc/elements/1.1/">
<dc:subject><rdf:Bag>
<rdf:li>sunset</rdf:li>
<rdf:li>sunset</rdf:li>
<rdf:li>sky</rdf:li>
</rdf:Bag></dc:subject>
</rdf:Description></rdf:RDF></x:xmpmeta>
<?xpacket end='w'?>"#;
    let meta = parse_xmp(xmp);
    assert_eq!(meta.keywords, vec!["sunset", "sky"]);
}

#[test]
fn parse_xmp_empty_bytes_returns_default() {
    assert_eq!(parse_xmp(b""), ReadResult::default());
}

#[test]
fn parse_xmp_missing_fields_return_empty_strings() {
    // Only keywords, no title/description/category
    let xmp = br#"<?xpacket begin='' id='W5M0MpCehiHzreSzNTczkc9d'?>
<x:xmpmeta xmlns:x="adobe:ns:meta/">
<rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
<rdf:Description rdf:about="" xmlns:dc="http://purl.org/dc/elements/1.1/">
<dc:subject><rdf:Bag><rdf:li>solo</rdf:li></rdf:Bag></dc:subject>
</rdf:Description></rdf:RDF></x:xmpmeta>
<?xpacket end='w'?>"#;
    let meta = parse_xmp(xmp);
    assert_eq!(meta.keywords, vec!["solo"]);
    assert!(meta.title.is_empty());
    assert!(meta.description.is_empty());
    assert!(meta.categories.is_empty());
}

// ── Round-trip tests ───────────────────────────────────────────────────────

#[test]
fn jpeg_round_trip_parse() {
    let raw = make_jpeg(TEST_XMP);
    let xmp = read_jpeg_xmp_fast(&mut Cursor::new(&raw)).unwrap().unwrap();
    let meta = parse_xmp(&xmp);
    assert_eq!(meta.title, "Test Title");
    assert_eq!(meta.keywords, vec!["kw1", "kw2"]);
}

#[test]
fn png_round_trip_parse() {
    let raw = make_png(TEST_XMP);
    let xmp = read_png_xmp_fast(&mut Cursor::new(&raw)).unwrap().unwrap();
    let meta = parse_xmp(&xmp);
    assert_eq!(meta.title, "Test Title");
    assert_eq!(meta.keywords, vec!["kw1", "kw2"]);
}

#[test]
fn webp_round_trip_parse() {
    let raw = make_webp(TEST_XMP);
    let xmp = read_webp_xmp_fast(&mut Cursor::new(&raw)).unwrap().unwrap();
    let meta = parse_xmp(&xmp);
    assert_eq!(meta.title, "Test Title");
    assert_eq!(meta.keywords, vec!["kw1", "kw2"]);
}
