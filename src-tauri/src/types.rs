use serde::{Deserialize, Serialize};

/// Payload sent from the frontend when saving.
/// `filepath` is the current full path; `filename` is the desired stem
/// (no extension, no directory). If the stem changed, the file is renamed.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveRequest {
    pub filepath: String,
    pub filename: String,
    pub title: String,
    pub description: String,
    pub keywords: Vec<String>,
    pub categories: String,
    pub release_filename: String,
}

/// XMP fields returned when reading an image.
#[derive(Serialize, Default, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReadResult {
    pub title: String,
    pub description: String,
    pub keywords: Vec<String>,
    pub categories: String,
    pub release_filename: String,
}
