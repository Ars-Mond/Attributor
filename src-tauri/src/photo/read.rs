//! Merge-read of metadata across EXIF, IPTC, and XMP with EXIF > IPTC > XMP precedence.
//! Reads stream via `little_exif`'s path API (`Seek + Read`); the whole file is never loaded.

use log::{debug, info, warn};

use little_exif::exif_tag::ExifTag;
use little_exif::iptc::IptcData;
use little_exif::metadata::Metadata as LeMetadata;

use super::xmp::parse_xmp_fields;
use super::{non_empty, split_semicolons, unique_keywords, Metadata, Photo};

// IPTC record 2 dataset numbers (decimal)
const R2: u8 = 2;
const DS_OBJECT_NAME: u8 = 5;
const DS_KEYWORDS: u8 = 25;
const DS_HEADLINE: u8 = 105;
const DS_CAPTION_ABSTRACT: u8 = 120;

pub(crate) fn read_metadata(photo: &Photo) -> Result<Metadata, String> {
    let path = photo.path();
    info!("read_metadata: {}", path.display());

    // Streaming Seek+Read of EXIF/IPTC/XMP via the fork's path API — no whole-file load.
    let le = LeMetadata::new_from_path(path).unwrap_or_else(|e| {
        warn!("read_metadata: parse failed for {}: {e}", path.display());
        LeMetadata::new()
    });
    let is_jpeg = photo.is_jpeg();

    // EXIF
    let exif_image_desc = tag_string(&le, 0x010e);
    let exif_xp_title = tag_utf16(&le, 0x9c9b);
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

    // Merge with EXIF > IPTC > XMP precedence; empty values never override populated ones.
    let title = exif_xp_title
        .or(iptc_object_name)
        .or(iptc_headline)
        .or(xmp_title)
        .or(xmp_headline)
        .unwrap_or_default();

    let description = exif_image_desc
        .or(exif_xp_subject)
        .or(iptc_caption)
        .or(xmp_desc)
        .unwrap_or_default();

    // Keywords are a set union across all blocks (precedence does not apply).
    let keywords = unique_keywords([
        exif_xp_keywords.map(split_semicolons).unwrap_or_default(),
        iptc_keywords,
        xmp_keywords,
    ]);

    // Category currently lives in XMP only.
    let category = xmp_category.unwrap_or_default();

    debug!(
        "read_metadata: title={:?} kw_count={} category={:?}",
        title,
        keywords.len(),
        category
    );

    Ok(Metadata { title, description, keywords, category })
}

// ── EXIF helpers ─────────────────────────────────────────────────────────────

fn tag_string(le: &LeMetadata, hex: u16) -> Option<String> {
    le.get_tag_by_hex(hex, None).next().and_then(|tag| match tag {
        ExifTag::ImageDescription(s) => non_empty(s.clone()),
        _ => None,
    })
}

fn tag_utf16(le: &LeMetadata, hex: u16) -> Option<String> {
    le.get_tag_by_hex(hex, None).next().and_then(|tag| {
        let s = match tag {
            ExifTag::XPTitle(v) => &v.0,
            ExifTag::XPKeywords(v) => &v.0,
            ExifTag::XPSubject(v) => &v.0,
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
