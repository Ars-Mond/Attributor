use attributor_lib::{parse_xmp, read_jpeg_xmp_fast, read_png_xmp_fast, read_webp_xmp_fast};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::io::Cursor;

// ── Image builders (same as in xmp_read.rs) ────────────────────────────────

const JPEG_XMP_HEADER: &[u8] = b"http://ns.adobe.com/xap/1.0/\0";
const PNG_SIG: [u8; 8] = [137, 80, 78, 71, 13, 10, 26, 10];
const PNG_XMP_KEYWORD: &[u8] = b"XML:com.adobe.xmp";

fn make_jpeg(xmp: &[u8]) -> Vec<u8> {
    let mut v = vec![0xFF, 0xD8];
    let data: Vec<u8> = [JPEG_XMP_HEADER, xmp].concat();
    let len = (data.len() + 2) as u16;
    v.push(0xFF); v.push(0xE1);
    v.extend_from_slice(&len.to_be_bytes());
    v.extend_from_slice(&data);
    v.extend_from_slice(&[0xFF, 0xDA]);
    v
}

/// JPEG with N non-XMP APP segments preceding the XMP APP1.
/// Tests the overhead of skipping many segments.
fn make_jpeg_with_prefix_segments(xmp: &[u8], n: usize) -> Vec<u8> {
    let mut v = vec![0xFF, 0xD8];
    for i in 0..n {
        // Fake APP2–APP13 segments with 512 bytes of padding
        let marker = 0xE2u8 + (i % 12) as u8;
        let data = vec![b'X'; 512];
        let len = (data.len() + 2) as u16;
        v.push(0xFF); v.push(marker);
        v.extend_from_slice(&len.to_be_bytes());
        v.extend_from_slice(&data);
    }
    // XMP APP1
    let seg: Vec<u8> = [JPEG_XMP_HEADER, xmp].concat();
    let len = (seg.len() + 2) as u16;
    v.push(0xFF); v.push(0xE1);
    v.extend_from_slice(&len.to_be_bytes());
    v.extend_from_slice(&seg);
    v.extend_from_slice(&[0xFF, 0xDA]);
    v
}

fn make_png(xmp: &[u8]) -> Vec<u8> {
    let mut v = PNG_SIG.to_vec();
    // iTXt chunk
    let mut data = Vec::new();
    data.extend_from_slice(PNG_XMP_KEYWORD);
    data.extend_from_slice(&[0x00, 0x00, 0x00, 0x00, 0x00]);
    data.extend_from_slice(xmp);
    v.extend_from_slice(&(data.len() as u32).to_be_bytes());
    v.extend_from_slice(b"iTXt");
    v.extend_from_slice(&data);
    v.extend_from_slice(&[0, 0, 0, 0]);
    // IDAT
    v.extend_from_slice(&[0, 0, 0, 0]);
    v.extend_from_slice(b"IDAT");
    v.extend_from_slice(&[0, 0, 0, 0]);
    v
}

fn make_webp(xmp: &[u8]) -> Vec<u8> {
    let padded = xmp.len() + (xmp.len() & 1);
    let mut v = Vec::new();
    v.extend_from_slice(b"RIFF");
    v.extend_from_slice(&(4u32 + 8 + padded as u32).to_le_bytes());
    v.extend_from_slice(b"WEBP");
    v.extend_from_slice(b"XMP ");
    v.extend_from_slice(&(xmp.len() as u32).to_le_bytes());
    v.extend_from_slice(xmp);
    if xmp.len() & 1 != 0 { v.push(0); }
    v
}

// ── XMP packet fixtures ────────────────────────────────────────────────────

fn small_xmp() -> Vec<u8> {
    b"<?xpacket begin='\xef\xbb\xbf' id='W5M0MpCehiHzreSzNTczkc9d'?>\n\
<x:xmpmeta xmlns:x=\"adobe:ns:meta/\">\
<rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\
<rdf:Description rdf:about=\"\" \
xmlns:dc=\"http://purl.org/dc/elements/1.1/\" \
xmlns:photoshop=\"http://ns.adobe.com/photoshop/1.0/\">\
<dc:title><rdf:Alt><rdf:li xml:lang=\"x-default\">Sunset</rdf:li></rdf:Alt></dc:title>\
<dc:subject><rdf:Bag><rdf:li>sunset</rdf:li><rdf:li>sky</rdf:li></rdf:Bag></dc:subject>\
<photoshop:Category>nature</photoshop:Category>\
</rdf:Description></rdf:RDF></x:xmpmeta>\n\
<?xpacket end='w'?>"
        .to_vec()
}

fn large_xmp() -> Vec<u8> {
    // Simulates a real stock photo XMP: long title, description, 50 keywords
    let mut s = String::from(
        "<?xpacket begin='\u{feff}' id='W5M0MpCehiHzreSzNTczkc9d'?>\n\
<x:xmpmeta xmlns:x=\"adobe:ns:meta/\">\
<rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\
<rdf:Description rdf:about=\"\" \
xmlns:dc=\"http://purl.org/dc/elements/1.1/\" \
xmlns:photoshop=\"http://ns.adobe.com/photoshop/1.0/\">\
<dc:title><rdf:Alt><rdf:li xml:lang=\"x-default\">Beautiful Mountain Landscape at Golden Hour</rdf:li></rdf:Alt></dc:title>\
<dc:description><rdf:Alt><rdf:li xml:lang=\"x-default\">\
A stunning natural landscape featuring majestic snow-capped mountains reflected in a crystal-clear alpine lake, \
surrounded by lush pine forests under a dramatic golden hour sky.\
</rdf:li></rdf:Alt></dc:description>\
<dc:subject><rdf:Bag>",
    );
    for i in 0..50 {
        s.push_str(&format!("<rdf:li>keyword number {i:02}</rdf:li>"));
    }
    s.push_str(
        "</rdf:Bag></dc:subject>\
<photoshop:Category>nature</photoshop:Category>\
</rdf:Description></rdf:RDF></x:xmpmeta>\n\
<?xpacket end='w'?>",
    );
    s.into_bytes()
}

// ── Benchmarks ─────────────────────────────────────────────────────────────

fn bench_jpeg(c: &mut Criterion) {
    let small = small_xmp();
    let large = large_xmp();

    let small_img = make_jpeg(&small);
    let large_img = make_jpeg(&large);
    let large_img_prefixed = make_jpeg_with_prefix_segments(&large, 8);

    let mut group = c.benchmark_group("jpeg_xmp_read");
    group.bench_with_input(
        BenchmarkId::new("small_xmp", small.len()),
        &small_img,
        |b, data| b.iter(|| read_jpeg_xmp_fast(&mut Cursor::new(black_box(data.as_slice())))),
    );
    group.bench_with_input(
        BenchmarkId::new("large_xmp", large.len()),
        &large_img,
        |b, data| b.iter(|| read_jpeg_xmp_fast(&mut Cursor::new(black_box(data.as_slice())))),
    );
    group.bench_with_input(
        BenchmarkId::new("large_xmp_8_prefix_segs", large.len()),
        &large_img_prefixed,
        |b, data| b.iter(|| read_jpeg_xmp_fast(&mut Cursor::new(black_box(data.as_slice())))),
    );
    group.finish();
}

fn bench_png(c: &mut Criterion) {
    let small = small_xmp();
    let large = large_xmp();

    let small_img = make_png(&small);
    let large_img = make_png(&large);

    let mut group = c.benchmark_group("png_xmp_read");
    group.bench_with_input(
        BenchmarkId::new("small_xmp", small.len()),
        &small_img,
        |b, data| b.iter(|| read_png_xmp_fast(&mut Cursor::new(black_box(data.as_slice())))),
    );
    group.bench_with_input(
        BenchmarkId::new("large_xmp", large.len()),
        &large_img,
        |b, data| b.iter(|| read_png_xmp_fast(&mut Cursor::new(black_box(data.as_slice())))),
    );
    group.finish();
}

fn bench_webp(c: &mut Criterion) {
    let small = small_xmp();
    let large = large_xmp();

    let small_img = make_webp(&small);
    let large_img = make_webp(&large);

    let mut group = c.benchmark_group("webp_xmp_read");
    group.bench_with_input(
        BenchmarkId::new("small_xmp", small.len()),
        &small_img,
        |b, data| b.iter(|| read_webp_xmp_fast(&mut Cursor::new(black_box(data.as_slice())))),
    );
    group.bench_with_input(
        BenchmarkId::new("large_xmp", large.len()),
        &large_img,
        |b, data| b.iter(|| read_webp_xmp_fast(&mut Cursor::new(black_box(data.as_slice())))),
    );
    group.finish();
}

fn bench_parse_xmp(c: &mut Criterion) {
    let small = small_xmp();
    let large = large_xmp();

    let mut group = c.benchmark_group("parse_xmp");
    group.bench_with_input(
        BenchmarkId::new("small", small.len()),
        &small,
        |b, data| b.iter(|| parse_xmp(black_box(data.as_slice()))),
    );
    group.bench_with_input(
        BenchmarkId::new("large_50_keywords", large.len()),
        &large,
        |b, data| b.iter(|| parse_xmp(black_box(data.as_slice()))),
    );
    group.finish();
}

criterion_group!(benches, bench_jpeg, bench_png, bench_webp, bench_parse_xmp);
criterion_main!(benches);
