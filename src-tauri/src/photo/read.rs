//! Merge-read of metadata across EXIF, IPTC, and XMP with EXIF > IPTC > XMP precedence.
//!
//! XMP is read for every format with a tolerant streaming scanner (`Seek + Read`),
//! independent of EXIF presence or WebP sub-format. EXIF and IPTC are read via the
//! `little_exif` path API for JPEG only: that API is all-or-nothing and rejects
//! XMP-only PNG/WebP (no EXIF / non-VP8X), so restricting it to JPEG keeps those
//! files readable and avoids the fork's internal "No metadata found" error logs.

use std::path::Path;

use log::{debug, info};

use little_exif::exif_tag::ExifTag;
use little_exif::iptc::IptcData;
use little_exif::metadata::Metadata as LeMetadata;

use super::xmp::{parse_xmp_fields, read_xmp_from_path};
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

    // EXIF + IPTC via little_exif — JPEG only (see module docs).
    let exif = if photo.is_jpeg() {
        read_exif_iptc(path)
    } else {
        ExifIptc::default()
    };

    // XMP via the tolerant streaming scanner — every format, independent of EXIF.
    let (xmp_title, xmp_headline, xmp_desc, xmp_keywords, xmp_category) =
        match read_xmp_from_path(path) {
            Some(bytes) => parse_xmp_fields(&bytes),
            None => (None, None, None, vec![], None),
        };

    // Merge with EXIF > IPTC > XMP precedence; empty values never override populated ones.
    let title = exif
        .xp_title
        .or(exif.iptc_object_name)
        .or(exif.iptc_headline)
        .or(xmp_title)
        .or(xmp_headline)
        .unwrap_or_default();

    let description = exif
        .image_desc
        .or(exif.xp_subject)
        .or(exif.iptc_caption)
        .or(xmp_desc)
        .unwrap_or_default();

    // Keywords are a set union across all blocks (precedence does not apply).
    let keywords = unique_keywords([
        exif.xp_keywords.map(split_semicolons).unwrap_or_default(),
        exif.iptc_keywords,
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

/// Best-effort EXIF + IPTC values extracted from a JPEG via `little_exif`.
#[derive(Default)]
struct ExifIptc {
    image_desc: Option<String>,
    xp_title: Option<String>,
    xp_keywords: Option<String>,
    xp_subject: Option<String>,
    iptc_object_name: Option<String>,
    iptc_headline: Option<String>,
    iptc_caption: Option<String>,
    iptc_keywords: Vec<String>,
}

fn read_exif_iptc(path: &Path) -> ExifIptc {
    // `new_from_path` is all-or-nothing: a JPEG without EXIF errors before IPTC is
    // read. Treat any failure as "no EXIF/IPTC" — XMP still carries the fields.
    let le = match LeMetadata::new_from_path(path) {
        Ok(le) => le,
        Err(e) => {
            debug!("read_metadata: no EXIF/IPTC for {}: {e}", path.display());
            return ExifIptc::default();
        }
    };

    let (iptc_object_name, iptc_headline, iptc_caption, iptc_keywords) =
        if let Some(iptc) = le.get_iptc() {
            (
                iptc_string(iptc, R2, DS_OBJECT_NAME),
                iptc_string(iptc, R2, DS_HEADLINE),
                iptc_string(iptc, R2, DS_CAPTION_ABSTRACT),
                iptc_all_strings(iptc, R2, DS_KEYWORDS),
            )
        } else {
            (None, None, None, vec![])
        };

    ExifIptc {
        image_desc: tag_string(&le, 0x010e),
        xp_title: tag_utf16(&le, 0x9c9b),
        xp_keywords: tag_utf16(&le, 0x9c9e),
        xp_subject: tag_utf16(&le, 0x9c9f),
        iptc_object_name,
        iptc_headline,
        iptc_caption,
        iptc_keywords,
    }
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
