//! Metadata rules of the MCP tools, mirroring the editor: keyword normalization, partial updates, file-name cleanup and the checks before saving.

use std::path::Path;

use serde::Deserialize;
use serde_json::{json, Value};

use crate::store::{MetadataResolution, StoredMetadata, SyncState};

/// Message for an image whose file changed outside the app after the store recorded it.
pub(crate) const CONFLICT: &str = "the file was changed outside Attributor after the app stored its metadata; call resolve_conflict first";

/// How `set_metadata` treats the keywords it receives.
#[derive(Deserialize, Default, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum KeywordsMode {
    #[default]
    Replace,
    Add,
    Remove,
}

/// Arguments of `set_metadata`: the images and the fields to change (absent fields stay as they are).
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct SetMetadataArgs {
    pub paths: Vec<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub keywords: Option<Vec<String>>,
    #[serde(default)]
    pub keywords_mode: KeywordsMode,
    pub categories: Option<String>,
    pub release_filename: Option<String>,
    pub editorial: Option<bool>,
    pub mature_content: Option<bool>,
    pub illustration: Option<bool>,
}

impl SetMetadataArgs {
    /// Whether the call changes nothing.
    pub(crate) fn changes_nothing(&self) -> bool {
        self.title.is_none()
            && self.description.is_none()
            && self.keywords.is_none()
            && self.categories.is_none()
            && self.release_filename.is_none()
            && self.editorial.is_none()
            && self.mature_content.is_none()
            && self.illustration.is_none()
    }

    /// `meta` with the requested fields changed.
    pub(crate) fn apply(&self, meta: &StoredMetadata) -> StoredMetadata {
        let mut out = meta.clone();
        if let Some(v) = &self.title {
            out.title = v.clone();
        }
        if let Some(v) = &self.description {
            out.description = v.clone();
        }
        if let Some(words) = &self.keywords {
            let given = normalize_keywords(words);
            out.keywords = match self.keywords_mode {
                KeywordsMode::Replace => given,
                KeywordsMode::Add => {
                    let mut list = meta.keywords.clone();
                    for word in given {
                        if !list.iter().any(|k| k.to_lowercase() == word) {
                            list.push(word);
                        }
                    }
                    list
                }
                KeywordsMode::Remove => meta
                    .keywords
                    .iter()
                    .filter(|k| !given.contains(&k.to_lowercase()))
                    .cloned()
                    .collect(),
            };
        }
        if let Some(v) = &self.categories {
            out.categories = v.clone();
        }
        if let Some(v) = &self.release_filename {
            out.release_filename = v.clone();
        }
        if let Some(v) = self.editorial {
            out.editorial = v;
        }
        if let Some(v) = self.mature_content {
            out.mature_content = v;
        }
        if let Some(v) = self.illustration {
            out.illustration = v;
        }
        out
    }
}

/// Keywords as the editor adds them: comma-separated entries split apart, trimmed, lower-cased, empty ones dropped and duplicates removed.
pub(crate) fn normalize_keywords(words: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for word in words.iter().flat_map(|w| w.split(',')) {
        let word = word.trim().to_lowercase();
        if !word.is_empty() && !out.contains(&word) {
            out.push(word);
        }
    }
    out
}

/// The editor's checks before Save: a file name, a title, a description and at least one keyword.
///
/// # Errors
///
/// Returns the failed checks joined into one message.
pub(crate) fn check_before_save(stem: &str, meta: &StoredMetadata) -> Result<(), String> {
    let missing: Vec<&str> = [
        (stem.trim().is_empty(), "a file name"),
        (meta.title.trim().is_empty(), "a title"),
        (meta.description.trim().is_empty(), "a description"),
        (meta.keywords.is_empty(), "at least one keyword"),
    ]
    .into_iter()
    .filter_map(|(failed, what)| failed.then_some(what))
    .collect();
    if missing.is_empty() {
        Ok(())
    } else {
        Err(format!("not saved: the image needs {}", missing.join(", ")))
    }
}

/// A new file name as the editor accepts it: trimmed, without an image extension (other dots belong to the name), and without characters that cannot appear in a file name.
///
/// # Errors
///
/// Returns a message when the name is empty or contains a path separator or a reserved character.
pub(crate) fn clean_file_name(name: &str) -> Result<String, String> {
    let trimmed = name.trim();
    let stem = match trimmed.rsplit_once('.') {
        Some((stem, ext))
            if !stem.is_empty()
                && matches!(
                    ext.to_ascii_lowercase().as_str(),
                    "jpg" | "jpeg" | "png" | "webp"
                ) =>
        {
            stem
        }
        _ => trimmed,
    };
    if stem.is_empty() || stem == "." || stem == ".." {
        return Err("fileName must not be empty".into());
    }
    if let Some(c) = stem
        .chars()
        .find(|c| c.is_control() || "\\/:*?\"<>|".contains(*c))
    {
        return Err(format!("fileName must not contain {c:?}"));
    }
    Ok(stem.to_string())
}

/// The file stem of `path`, which the editor shows as the file name.
pub(crate) fn stem_of(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// The editor fields of an image, as the tools return them.
pub(crate) fn metadata_json(path: &str, meta: &StoredMetadata) -> Value {
    let mut value = serde_json::to_value(meta).unwrap_or_else(|_| json!({}));
    if let Some(obj) = value.as_object_mut() {
        obj.shift_insert(0, "fileName".to_string(), json!(stem_of(path)));
    }
    value
}

/// Name of a sync state in tool results.
pub(crate) fn status_name(state: SyncState) -> &'static str {
    match state {
        SyncState::Synced => "synced",
        SyncState::AppOnly => "appOnly",
    }
}

/// A resolved record as a tool result item.
pub(crate) fn resolution_json(path: &str, resolution: &MetadataResolution) -> Value {
    match resolution {
        MetadataResolution::Resolved {
            metadata,
            sync_state,
        } => json!({
            "path": path,
            "status": status_name(*sync_state),
            "metadata": metadata_json(path, metadata),
        }),
        MetadataResolution::Conflict { store, file } => json!({
            "path": path,
            "status": "conflict",
            "app": metadata_json(path, store),
            "file": metadata_json(path, file),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meta(keywords: &[&str]) -> StoredMetadata {
        StoredMetadata {
            title: "T".into(),
            description: "D".into(),
            keywords: keywords.iter().map(|k| k.to_string()).collect(),
            release_filename: "release.pdf".into(),
            ..Default::default()
        }
    }

    fn args(json: Value) -> SetMetadataArgs {
        serde_json::from_value(json).unwrap()
    }

    #[test]
    fn keywords_are_normalized_like_the_editor() {
        let words = vec![
            " Sky ".to_string(),
            "sea, Sand".into(),
            "sky".into(),
            "".into(),
        ];
        assert_eq!(normalize_keywords(&words), ["sky", "sea", "sand"]);
    }

    #[test]
    fn keyword_modes() {
        let current = meta(&["Beach", "sun"]);
        let replace = args(json!({"paths": [], "keywords": ["A", "b"]}));
        assert_eq!(replace.apply(&current).keywords, ["a", "b"]);
        let add = args(json!({"paths": [], "keywords": ["beach", "Sea"], "keywordsMode": "add"}));
        assert_eq!(add.apply(&current).keywords, ["Beach", "sun", "sea"]);
        let remove = args(json!({"paths": [], "keywords": ["BEACH"], "keywordsMode": "remove"}));
        assert_eq!(remove.apply(&current).keywords, ["sun"]);
    }

    #[test]
    fn only_given_fields_change() {
        let current = meta(&["a"]);
        let patch = args(json!({"paths": [], "title": "New", "editorial": true}));
        let out = patch.apply(&current);
        assert_eq!(out.title, "New");
        assert!(out.editorial);
        assert_eq!(out.description, "D");
        assert_eq!(out.release_filename, "release.pdf");
        assert_eq!(out.keywords, ["a"]);
        assert!(!patch.changes_nothing());
        assert!(args(json!({"paths": []})).changes_nothing());
    }

    #[test]
    fn unknown_fields_are_rejected() {
        let err = serde_json::from_value::<SetMetadataArgs>(json!({"paths": [], "fileName": "x"}))
            .unwrap_err();
        assert!(err.to_string().contains("fileName"));
    }

    #[test]
    fn save_checks_match_the_editor() {
        assert!(check_before_save("photo", &meta(&["a"])).is_ok());
        let err = check_before_save(" ", &StoredMetadata::default()).unwrap_err();
        assert_eq!(
            err,
            "not saved: the image needs a file name, a title, a description, at least one keyword"
        );
    }

    #[test]
    fn file_names_are_cleaned() {
        assert_eq!(clean_file_name(" beach-01.jpg ").unwrap(), "beach-01");
        assert_eq!(clean_file_name("beach").unwrap(), "beach");
        assert_eq!(clean_file_name("IMG_1.edit").unwrap(), "IMG_1.edit");
        assert_eq!(clean_file_name("IMG_1.edit.PNG").unwrap(), "IMG_1.edit");
        assert_eq!(clean_file_name(".hidden").unwrap(), ".hidden");
        assert!(clean_file_name("..").is_err());
        assert!(clean_file_name("a/b").is_err());
        assert!(clean_file_name("..\\up").is_err());
        assert!(clean_file_name("").is_err());
    }

    #[test]
    fn metadata_json_leads_with_the_file_name() {
        let value = metadata_json("C:/photos/beach.jpg", &meta(&["a"]));
        assert_eq!(
            value.as_object().unwrap().keys().next().unwrap(),
            "fileName"
        );
        assert_eq!(value["fileName"], "beach");
        assert_eq!(value["releaseFilename"], "release.pdf");
    }
}
