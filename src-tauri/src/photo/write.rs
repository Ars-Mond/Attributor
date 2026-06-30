//! Multi-block write: duplicates the managed fields into every supported block
//! (EXIF, IPTC, XMP) and removes fields the user cleared. Unrelated EXIF and IPTC
//! tags are preserved. Persisted via `little_exif`'s `write_to_file`.

use log::{info, warn};

use little_exif::exif_tag::ExifTag;
use little_exif::exif_tag_format::Utf16String;
use little_exif::iptc::IptcData;
use little_exif::metadata::Metadata as LeMetadata;
use little_exif::xmp::XmpData;

use super::xmp::build_xmp_packet;
use super::{Metadata, Photo};

const R2: u8 = 2;
const DS_OBJECT_NAME: u8 = 5;
const DS_KEYWORDS: u8 = 25;
const DS_HEADLINE: u8 = 105;
const DS_CAPTION_ABSTRACT: u8 = 120;

pub(crate) fn save_metadata(photo: &Photo, meta: &Metadata) -> Result<(), String> {
    let path = photo.path();
    info!("save_metadata: {}", path.display());

    // Load existing metadata (streaming) so unrelated EXIF/IPTC tags are preserved.
    let mut le = LeMetadata::new_from_path(path).unwrap_or_else(|e| {
        warn!("save_metadata: parse failed, starting fresh: {e}");
        LeMetadata::new()
    });

    write_exif(&mut le, meta);
    if photo.is_jpeg() {
        write_iptc(&mut le, meta);
    }
    write_xmp(&mut le, meta);

    le.write_to_file(path).map_err(|e| e.to_string())?;
    info!("save_metadata: done → {}", path.display());
    Ok(())
}

// ── EXIF ───────────────────────────────────────────────────────────────────

fn write_exif(le: &mut LeMetadata, m: &Metadata) {
    // title → XPTitle
    if m.title.is_empty() {
        le.remove_tag(ExifTag::XPTitle(Utf16String::from("")));
    } else {
        le.set_tag(ExifTag::XPTitle(Utf16String::from(m.title.as_str())));
    }

    // description → ImageDescription + XPSubject
    if m.description.is_empty() {
        le.remove_tag(ExifTag::ImageDescription(String::new()));
        le.remove_tag(ExifTag::XPSubject(Utf16String::from("")));
    } else {
        le.set_tag(ExifTag::ImageDescription(m.description.clone()));
        le.set_tag(ExifTag::XPSubject(Utf16String::from(m.description.as_str())));
    }

    // keywords → XPKeywords (semicolon-joined)
    if m.keywords.is_empty() {
        le.remove_tag(ExifTag::XPKeywords(Utf16String::from("")));
    } else {
        le.set_tag(ExifTag::XPKeywords(Utf16String::from(m.keywords.join(";").as_str())));
    }
}

// ── IPTC (JPEG only) ─────────────────────────────────────────────────────────

fn write_iptc(le: &mut LeMetadata, m: &Metadata) {
    // Preserve unrelated IPTC datasets: start from the existing block, drop only the
    // managed datasets, then re-add the non-empty values.
    let mut iptc = le.get_iptc().cloned().unwrap_or_else(IptcData::new);
    iptc.remove_fields(R2, DS_OBJECT_NAME);
    iptc.remove_fields(R2, DS_HEADLINE);
    iptc.remove_fields(R2, DS_CAPTION_ABSTRACT);
    iptc.remove_fields(R2, DS_KEYWORDS);

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

    if iptc.fields.is_empty() {
        le.clear_iptc();
    } else {
        le.set_iptc(iptc);
    }
}

// ── XMP ──────────────────────────────────────────────────────────────────────

fn write_xmp(le: &mut LeMetadata, m: &Metadata) {
    let has_content = !m.title.is_empty()
        || !m.description.is_empty()
        || !m.keywords.is_empty()
        || !m.category.is_empty();

    if has_content {
        le.set_xmp(XmpData::from_raw(build_xmp_packet(m)));
    } else {
        le.clear_xmp();
    }
}
