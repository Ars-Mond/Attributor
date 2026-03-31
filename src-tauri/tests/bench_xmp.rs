use attributor_lib::{
    old_read_xmp, parse_xmp, read_jpeg_xmp_fast, read_png_xmp_fast, read_webp_xmp_fast,
};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::io::Cursor;

// ── XMP packet fixtures ────────────────────────────────────────────────────

const JPEG_XMP_HEADER: &[u8] = b"http://ns.adobe.com/xap/1.0/\0";
const PNG_SIG: [u8; 8] = [137, 80, 78, 71, 13, 10, 26, 10];
const PNG_XMP_KEYWORD: &[u8] = b"XML:com.adobe.xmp";

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
    let mut s = String::from(
        "<?xpacket begin='\u{feff}' id='W5M0MpCehiHzreSzNTczkc9d'?>\n\
<x:xmpmeta xmlns:x=\"adobe:ns:meta/\">\
<rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\
<rdf:Description rdf:about=\"\" \
xmlns:dc=\"http://purl.org/dc/elements/1.1/\" \
xmlns:photoshop=\"http://ns.adobe.com/photoshop/1.0/\">\
<dc:title><rdf:Alt><rdf:li xml:lang=\"x-default\">Beautiful Mountain Landscape at Golden Hour</rdf:li></rdf:Alt></dc:title>\
<dc:description><rdf:Alt><rdf:li xml:lang=\"x-default\">\
A stunning natural landscape featuring majestic snow-capped mountains.\
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

// ── Image builders ─────────────────────────────────────────────────────────
//
// "payload" = fake image data appended after the XMP to simulate a real file.
// JPEG:  APP2 segments after the XMP APP1, then SOS (stops fast reader).
//        Old reader parses all APP2 segments; fast reader stops at first APP1 XMP.
// PNG:   large IDAT chunk after the iTXt XMP.
//        Old reader reads all IDAT bytes; fast reader stops at the IDAT header.
// WebP:  VP8L chunk (image data) BEFORE XMP, as in real WebP files.
//        Old reader reads all VP8L bytes; fast reader seeks past it.

// ── JPEG builders ─────────────────────────────────────────────────────────

fn make_jpeg(xmp: &[u8], payload_bytes: usize) -> Vec<u8> {
    let mut v = vec![0xFF, 0xD8]; // SOI
    // APP1 XMP — placed early, before payload
    let seg: Vec<u8> = [JPEG_XMP_HEADER, xmp].concat();
    let len = (seg.len() + 2) as u16;
    v.push(0xFF); v.push(0xE1);
    v.extend_from_slice(&len.to_be_bytes());
    v.extend_from_slice(&seg);
    // Simulated ICC / thumbnail APP2 segments that come after XMP in a real file.
    // Each APP2 segment holds at most 65,533 bytes of data.
    let mut remaining = payload_bytes;
    while remaining > 0 {
        let chunk = remaining.min(65_533);
        let slen = (chunk + 2) as u16;
        v.push(0xFF); v.push(0xE2);
        v.extend_from_slice(&slen.to_be_bytes());
        v.extend(std::iter::repeat(0xABu8).take(chunk));
        remaining -= chunk;
    }
    v.push(0xFF); v.push(0xDA); // SOS — fast reader stops here
    v
}

// ── PNG builders ───────────────────────────────────────────────────────────

fn png_chunk(v: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    v.extend_from_slice(&(data.len() as u32).to_be_bytes());
    v.extend_from_slice(kind);
    v.extend_from_slice(data);
    v.extend_from_slice(&[0, 0, 0, 0]); // CRC not validated
}

fn make_png(xmp: &[u8], payload_bytes: usize) -> Vec<u8> {
    let mut v = PNG_SIG.to_vec();
    // iTXt XMP chunk (before IDAT)
    let mut itxt: Vec<u8> = Vec::new();
    itxt.extend_from_slice(PNG_XMP_KEYWORD);
    itxt.extend_from_slice(&[0x00, 0x00, 0x00, 0x00, 0x00]);
    itxt.extend_from_slice(xmp);
    png_chunk(&mut v, b"iTXt", &itxt);
    // Large IDAT chunk — fast reader stops at the IDAT *header* without reading data
    let idat_data: Vec<u8> = std::iter::repeat(0xABu8).take(payload_bytes).collect();
    png_chunk(&mut v, b"IDAT", &idat_data);
    png_chunk(&mut v, b"IEND", &[]);
    v
}

// ── WebP builders ──────────────────────────────────────────────────────────

fn make_webp(xmp: &[u8], payload_bytes: usize) -> Vec<u8> {
    // Real WebP structure: VP8L (image data) comes BEFORE XMP.
    // Fast reader seeks past VP8L; old reader allocates it into DynImage.
    let vp8l_padded = payload_bytes + (payload_bytes & 1);
    let xmp_padded = xmp.len() + (xmp.len() & 1);
    let file_size = 4u32                   // "WEBP"
        + 8 + vp8l_padded as u32           // VP8L chunk
        + 8 + xmp_padded as u32;           // XMP  chunk

    let mut v = Vec::new();
    v.extend_from_slice(b"RIFF");
    v.extend_from_slice(&file_size.to_le_bytes());
    v.extend_from_slice(b"WEBP");
    // VP8L image data chunk
    v.extend_from_slice(b"VP8L");
    v.extend_from_slice(&(payload_bytes as u32).to_le_bytes());
    v.extend(std::iter::repeat(0xABu8).take(payload_bytes));
    if payload_bytes & 1 != 0 { v.push(0); }
    // XMP chunk
    v.extend_from_slice(b"XMP ");
    v.extend_from_slice(&(xmp.len() as u32).to_le_bytes());
    v.extend_from_slice(xmp);
    if xmp.len() & 1 != 0 { v.push(0); }
    v
}

// ── Benchmark groups ───────────────────────────────────────────────────────

const PAYLOAD_SMALL:  usize = 0;           // XMP-only, no image data
const PAYLOAD_MEDIUM: usize = 512 * 1024;  // 512 KB
const PAYLOAD_LARGE:  usize = 5 * 1024 * 1024; // 5 MB

fn bench_jpeg_old_vs_new(c: &mut Criterion) {
    let xmp = large_xmp();
    let datasets = [
        ("small",  make_jpeg(&xmp, PAYLOAD_SMALL)),
        ("512kb",  make_jpeg(&xmp, PAYLOAD_MEDIUM)),
        ("5mb",    make_jpeg(&xmp, PAYLOAD_LARGE)),
    ];

    let mut group = c.benchmark_group("jpeg_old_vs_new");
    for (label, data) in &datasets {
        group.throughput(Throughput::Bytes(data.len() as u64));
        group.bench_with_input(BenchmarkId::new("old", label), data, |b, d| {
            b.iter(|| old_read_xmp(black_box(d.as_slice())))
        });
        group.bench_with_input(BenchmarkId::new("new", label), data, |b, d| {
            b.iter(|| read_jpeg_xmp_fast(&mut Cursor::new(black_box(d.as_slice()))))
        });
    }
    group.finish();
}

fn bench_png_old_vs_new(c: &mut Criterion) {
    let xmp = large_xmp();
    let datasets = [
        ("small",  make_png(&xmp, PAYLOAD_SMALL)),
        ("512kb",  make_png(&xmp, PAYLOAD_MEDIUM)),
        ("5mb",    make_png(&xmp, PAYLOAD_LARGE)),
    ];

    let mut group = c.benchmark_group("png_old_vs_new");
    for (label, data) in &datasets {
        group.throughput(Throughput::Bytes(data.len() as u64));
        group.bench_with_input(BenchmarkId::new("old", label), data, |b, d| {
            b.iter(|| old_read_xmp(black_box(d.as_slice())))
        });
        group.bench_with_input(BenchmarkId::new("new", label), data, |b, d| {
            b.iter(|| read_png_xmp_fast(&mut Cursor::new(black_box(d.as_slice()))))
        });
    }
    group.finish();
}

fn bench_webp_old_vs_new(c: &mut Criterion) {
    let xmp = large_xmp();
    let datasets = [
        ("small",  make_webp(&xmp, PAYLOAD_SMALL)),
        ("512kb",  make_webp(&xmp, PAYLOAD_MEDIUM)),
        ("5mb",    make_webp(&xmp, PAYLOAD_LARGE)),
    ];

    let mut group = c.benchmark_group("webp_old_vs_new");
    for (label, data) in &datasets {
        group.throughput(Throughput::Bytes(data.len() as u64));
        group.bench_with_input(BenchmarkId::new("old", label), data, |b, d| {
            b.iter(|| old_read_xmp(black_box(d.as_slice())))
        });
        group.bench_with_input(BenchmarkId::new("new", label), data, |b, d| {
            b.iter(|| read_webp_xmp_fast(&mut Cursor::new(black_box(d.as_slice()))))
        });
    }
    group.finish();
}

fn bench_parse_xmp(c: &mut Criterion) {
    let small = small_xmp();
    let large = large_xmp();

    let mut group = c.benchmark_group("parse_xmp");
    group.bench_with_input(BenchmarkId::new("small", small.len()), &small, |b, d| {
        b.iter(|| parse_xmp(black_box(d.as_slice())))
    });
    group.bench_with_input(BenchmarkId::new("large_50_keywords", large.len()), &large, |b, d| {
        b.iter(|| parse_xmp(black_box(d.as_slice())))
    });
    group.finish();
}

criterion_group!(
    benches,
    bench_jpeg_old_vs_new,
    bench_png_old_vs_new,
    bench_webp_old_vs_new,
    bench_parse_xmp,
);
criterion_main!(benches);
