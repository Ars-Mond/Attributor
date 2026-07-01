//! Maps a configured field + a photo's stored metadata to its CSV cell string (research R10). Values
//! come only from the store; the file name is the one non-store value, derived from the path.

use std::path::Path;

use crate::store::StoredMetadata;

use super::{AppValueType, CsvField};

/// The cell string for `field` given the photo `path` and its stored `meta`.
pub fn cell(field: &CsvField, path: &str, meta: &StoredMetadata) -> String {
    match field.value_type {
        AppValueType::None => field.default_value.clone(),
        AppValueType::FileName => file_name(path),
        AppValueType::Title => meta.title.clone(),
        AppValueType::Description => meta.description.clone(),
        AppValueType::Keywords => meta.keywords.join(","),
        AppValueType::Category => normalize_category(&meta.categories),
        AppValueType::ReleaseFilename => meta.release_filename.clone(),
        AppValueType::Editorial => field.bool_format.render(meta.editorial).to_string(),
        AppValueType::MatureContent => field.bool_format.render(meta.mature_content).to_string(),
        AppValueType::Illustration => field.bool_format.render(meta.illustration).to_string(),
    }
}

fn file_name(path: &str) -> String {
    Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Normalize the stored `categories` string to a comma separator (I2): split on commas, trim each,
/// drop empties, re-join with a comma — matching the in-cell keyword convention (FR-012).
fn normalize_category(categories: &str) -> String {
    categories
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(",")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::csv::BoolFormat;

    fn meta() -> StoredMetadata {
        StoredMetadata {
            title: "T".into(),
            description: "D".into(),
            keywords: vec!["k1".into(), "k2".into()],
            categories: "Nature, People".into(),
            release_filename: "r.pdf".into(),
            editorial: true,
            mature_content: false,
            illustration: true,
        }
    }

    fn field(vt: AppValueType) -> CsvField {
        CsvField {
            csv_column: "c".into(),
            value_type: vt,
            default_value: String::new(),
            bool_format: BoolFormat::YesNo,
        }
    }

    #[test]
    fn maps_text_fields() {
        let m = meta();
        assert_eq!(cell(&field(AppValueType::Title), "x", &m), "T");
        assert_eq!(cell(&field(AppValueType::Description), "x", &m), "D");
        assert_eq!(cell(&field(AppValueType::ReleaseFilename), "x", &m), "r.pdf");
    }

    #[test]
    fn file_name_is_basename_with_ext() {
        let got = cell(&field(AppValueType::FileName), "/a/b/photo.jpg", &meta());
        assert_eq!(got, "photo.jpg");
    }

    #[test]
    fn keywords_join_with_comma() {
        assert_eq!(cell(&field(AppValueType::Keywords), "x", &meta()), "k1,k2");
    }

    #[test]
    fn category_normalized_to_comma() {
        assert_eq!(cell(&field(AppValueType::Category), "x", &meta()), "Nature,People");
    }

    #[test]
    fn none_emits_default_value() {
        let mut f = field(AppValueType::None);
        f.default_value = "RF".into();
        assert_eq!(cell(&f, "x", &meta()), "RF");
    }

    #[test]
    fn bool_formats_render_per_choice() {
        let m = meta();
        let mut yn = field(AppValueType::Editorial);
        yn.bool_format = BoolFormat::YesNo;
        assert_eq!(cell(&yn, "x", &m), "yes"); // editorial = true
        let mut tf = field(AppValueType::MatureContent);
        tf.bool_format = BoolFormat::TrueFalse;
        assert_eq!(cell(&tf, "x", &m), "false"); // mature_content = false
    }

    #[test]
    fn empty_values_are_empty() {
        let m = StoredMetadata::default();
        assert_eq!(cell(&field(AppValueType::Title), "x", &m), "");
        assert_eq!(cell(&field(AppValueType::Keywords), "x", &m), "");
        assert_eq!(cell(&field(AppValueType::Category), "x", &m), "");
    }
}
