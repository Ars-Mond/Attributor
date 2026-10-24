//! The static tool catalogue: names, descriptions, input schemas and annotations.
//! The server and the stdio bridge both answer `tools/list` from it, so the bridge needs no running app for discovery.

use serde_json::{json, Value};

/// Most images a metadata tool accepts per call.
pub(crate) const MAX_PATHS: usize = 500;
/// Most images a job tool (save, attribution) accepts per call.
pub(crate) const MAX_JOB_PATHS: usize = 1000;
/// Most small previews per call.
pub(crate) const MAX_SMALL_PREVIEWS: usize = 16;
/// Most large previews per call.
pub(crate) const MAX_LARGE_PREVIEWS: usize = 4;
/// Longest side of a small preview, matching the app's small thumbnails.
pub(crate) const SMALL_PREVIEW_EDGE: u32 = 360;
/// Longest side of a large preview; vision models gain nothing from more.
pub(crate) const LARGE_PREVIEW_EDGE: u32 = 1568;
/// How long a job tool waits for its job by default before returning the job id.
pub(crate) const DEFAULT_WAIT_SECS: u64 = 40;
/// Longest wait a caller may request; with the work around it, a call stays within the 60-second tool timeout many clients use.
pub(crate) const MAX_WAIT_SECS: u64 = 50;
/// Provider ids accepted by `list_ai_models` (the `ai::types::Provider` serde names).
pub(crate) const PROVIDER_IDS: [&str; 9] = [
    "ollama",
    "llamacpp",
    "lmstudio",
    "jan",
    "vllm",
    "unsloth",
    "openai",
    "chatgpt",
    "anthropic",
];

/// One tool of the catalogue.
struct ToolDef {
    name: &'static str,
    title: &'static str,
    description: &'static str,
    read_only: bool,
    destructive: bool,
    idempotent: bool,
    /// Whether the tool reaches services outside the app (the AI providers).
    open_world: bool,
    schema: fn() -> Value,
}

const TOOLS: &[ToolDef] = &[
    ToolDef {
        name: "get_status",
        title: "Get Attributor status",
        description: "Shows what is open in Attributor: the folder, the image opened in the editor, the selected images (batch mode when more than one), whether the editor has unsaved changes, and the AI provider and model that attribute_images uses.",
        read_only: true,
        destructive: false,
        idempotent: true,
        open_world: false,
        schema: no_arguments,
    },
    ToolDef {
        name: "list_images",
        title: "List images",
        description: "Lists the images (JPEG, PNG, WebP) of the folder open in Attributor, in the order the app shows them. Subfolders are included when the app's \"Read nested folders\" setting is on. Use offset and limit to page through large folders.",
        read_only: true,
        destructive: false,
        idempotent: true,
        open_world: false,
        schema: list_images_schema,
    },
    ToolDef {
        name: "get_metadata",
        title: "Get image metadata",
        description: "Reads the metadata Attributor shows for each image: fileName (without extension), title, description, keywords, categories, releaseFilename and the editorial, matureContent and illustration flags. status is \"synced\" when the file holds these values, \"appOnly\" when Attributor holds changes that are not written to the file yet (save_metadata writes them), or \"conflict\" when the file was changed outside Attributor after the app stored its version; a conflict returns both versions, and resolve_conflict picks one.",
        read_only: true,
        destructive: false,
        idempotent: true,
        open_world: false,
        schema: paths_schema,
    },
    ToolDef {
        name: "get_preview",
        title: "Get image previews",
        description: "Returns previews of the images so you can see them. size \"small\" is about 360 px on the long side (up to 16 images per call); \"large\" is up to 1568 px (up to 4 images per call). EXIF orientation is applied.",
        read_only: true,
        destructive: false,
        idempotent: true,
        open_world: false,
        schema: preview_schema,
    },
    ToolDef {
        name: "open_folder",
        title: "Open a folder",
        description: "Opens a folder of images in Attributor, like File > Open folder. Edits pending in the editor are kept in Attributor first. Returns the folder and its image count; a very large folder may still be opening when the call returns (status \"opening\"), so check get_status a moment later.",
        read_only: false,
        destructive: false,
        idempotent: true,
        open_world: false,
        schema: folder_schema,
    },
    ToolDef {
        name: "select_images",
        title: "Show images to the user",
        description: "Selects images in Attributor's file list so the user sees them: one path opens that image in the editor and viewer, several paths select them together (batch mode). The images must belong to the open folder. Other tools work with any path, so selecting is never required.",
        read_only: false,
        destructive: false,
        idempotent: true,
        open_world: false,
        schema: paths_schema,
    },
    ToolDef {
        name: "set_metadata",
        title: "Change image metadata",
        description: "Changes the metadata of one or more images in Attributor: the fields the editor offers. Only the fields you pass change. Changes stay in Attributor (status \"appOnly\") until save_metadata writes them into the files. Keywords are normalized as in the editor (trimmed, lower-case, no duplicates); keywordsMode \"replace\" (default) sets the list, \"add\" appends to it, \"remove\" deletes the given keywords. With several paths the same change applies to every image (batch editing). Images in conflict are skipped (see resolve_conflict).",
        read_only: false,
        destructive: true,
        idempotent: true,
        open_world: false,
        schema: set_metadata_schema,
    },
    ToolDef {
        name: "save_metadata",
        title: "Save metadata to files",
        description: "Writes the metadata Attributor holds into the image files (title, description, keywords and categories go to EXIF, IPTC and XMP), exactly like the editor's Save button and after the same checks: a file name, a title, a description and at least one keyword are required. fileName renames the file and is allowed with a single path only (give the name without extension). If the work takes longer than waitSeconds, the result is a jobId to follow with get_job.",
        read_only: false,
        destructive: true,
        idempotent: true,
        open_world: false,
        schema: save_schema,
    },
    ToolDef {
        name: "revert_metadata",
        title: "Revert to file metadata",
        description: "Discards the changes Attributor holds for the images and reloads the metadata from the files, like the editor's revert button. releaseFilename and the editorial, matureContent and illustration flags, which exist only in Attributor, are cleared.",
        read_only: false,
        destructive: true,
        idempotent: true,
        open_world: false,
        schema: paths_schema,
    },
    ToolDef {
        name: "resolve_conflict",
        title: "Resolve a metadata conflict",
        description: "Resolves a conflict reported by get_metadata, where the file was changed outside Attributor after the app stored its version. keep \"app\" keeps Attributor's version; keep \"file\" takes the file's version and keeps releaseFilename and the flags.",
        read_only: false,
        destructive: true,
        idempotent: true,
        open_world: false,
        schema: resolve_schema,
    },
    ToolDef {
        name: "attribute_images",
        title: "Attribute images with AI",
        description: "Fills title, description, keywords, categories and the editorial, matureContent and illustration flags of the images with Attributor's AI model, using the provider, model and profile set in Attributor (see get_status). New keywords are added to the existing ones; the other fields are replaced. Results stay in Attributor (status \"appOnly\") until save_metadata writes them. Images are processed one at a time; if the work takes longer than waitSeconds, the result is a jobId to follow with get_job.",
        read_only: false,
        destructive: true,
        idempotent: false,
        open_world: true,
        schema: attribute_schema,
    },
    ToolDef {
        name: "get_job",
        title: "Get job progress",
        description: "Reports the progress and the per-image results of a job started by save_metadata or attribute_images, waiting up to waitSeconds for it to finish.",
        read_only: true,
        destructive: false,
        idempotent: true,
        open_world: false,
        schema: job_wait_schema,
    },
    ToolDef {
        name: "cancel_job",
        title: "Cancel a job",
        description: "Cancels a running job. Images that are already done keep their results.",
        read_only: false,
        destructive: false,
        idempotent: true,
        open_world: false,
        schema: job_schema,
    },
    ToolDef {
        name: "get_settings",
        title: "Get settings",
        description: "Lists Attributor's settings with their current values, defaults and allowed values. MCP settings and credentials are not included. aiSettingsWritable tells whether update_settings may change AI settings; the user decides that in Attributor > Settings > MCP.",
        read_only: true,
        destructive: false,
        idempotent: true,
        open_world: false,
        schema: no_arguments,
    },
    ToolDef {
        name: "update_settings",
        title: "Change settings",
        description: "Changes Attributor's settings as its Settings dialog does. Pass setting keys from get_settings with their new values; every value is checked first, and nothing changes if one is invalid. AI settings (the \"ai\" and \"model-profiles\" sections, such as ai.provider, <provider>.baseUrl, <provider>.activeModel and ai.modelProfiles) change only when the user allows it in Settings > MCP. API keys and sign-ins are never available through MCP.",
        read_only: false,
        destructive: true,
        idempotent: true,
        open_world: false,
        schema: settings_schema,
    },
    ToolDef {
        name: "list_ai_models",
        title: "List AI models",
        description: "Lists the models an AI provider offers (installed, served or available to the account) and the provider's active model. Without provider, the provider selected in Attributor is used.",
        read_only: true,
        destructive: false,
        idempotent: true,
        open_world: true,
        schema: models_schema,
    },
];

/// The catalogue as `tools/list` returns it, in a fixed order.
pub(crate) fn catalogue() -> Vec<Value> {
    TOOLS.iter().map(tool_json).collect()
}

/// Whether the catalogue has a tool named `name`.
pub(crate) fn is_known(name: &str) -> bool {
    TOOLS.iter().any(|t| t.name == name)
}

fn tool_json(t: &ToolDef) -> Value {
    json!({
        "name": t.name,
        "title": t.title,
        "description": t.description,
        "inputSchema": (t.schema)(),
        "annotations": {
            "title": t.title,
            "readOnlyHint": t.read_only,
            "destructiveHint": t.destructive,
            "idempotentHint": t.idempotent,
            "openWorldHint": t.open_world,
        },
    })
}

fn object(properties: Value, required: &[&str]) -> Value {
    json!({
        "type": "object",
        "properties": properties,
        "required": required,
        "additionalProperties": false,
    })
}

fn no_arguments() -> Value {
    json!({"type": "object", "additionalProperties": false})
}

fn paths_property(max: usize) -> Value {
    json!({
        "type": "array",
        "description": "Absolute paths of the image files.",
        "items": {"type": "string"},
        "minItems": 1,
        "maxItems": max,
    })
}

fn wait_property(default: u64) -> Value {
    json!({
        "type": "integer",
        "description": "Seconds to wait for the job to finish before returning its jobId.",
        "minimum": 0,
        "maximum": MAX_WAIT_SECS,
        "default": default,
    })
}

fn paths_schema() -> Value {
    object(json!({"paths": paths_property(MAX_PATHS)}), &["paths"])
}

fn list_images_schema() -> Value {
    object(
        json!({
            "offset": {"type": "integer", "minimum": 0, "default": 0, "description": "Number of images to skip."},
            "limit": {"type": "integer", "minimum": 1, "maximum": 1000, "default": 200, "description": "Most images to return."},
        }),
        &[],
    )
}

fn preview_schema() -> Value {
    object(
        json!({
            "paths": paths_property(MAX_SMALL_PREVIEWS),
            "size": {"type": "string", "enum": ["small", "large"], "default": "small"},
        }),
        &["paths"],
    )
}

fn folder_schema() -> Value {
    object(
        json!({"path": {"type": "string", "description": "Absolute path of the folder."}}),
        &["path"],
    )
}

fn set_metadata_schema() -> Value {
    object(
        json!({
            "paths": paths_property(MAX_PATHS),
            "title": {"type": "string"},
            "description": {"type": "string"},
            "keywords": {"type": "array", "items": {"type": "string"}},
            "keywordsMode": {"type": "string", "enum": ["replace", "add", "remove"], "default": "replace"},
            "categories": {"type": "string", "description": "Comma-separated categories, as in the editor."},
            "releaseFilename": {"type": "string", "description": "File name of the model or property release."},
            "editorial": {"type": "boolean"},
            "matureContent": {"type": "boolean"},
            "illustration": {"type": "boolean"},
        }),
        &["paths"],
    )
}

fn save_schema() -> Value {
    object(
        json!({
            "paths": paths_property(MAX_JOB_PATHS),
            "fileName": {"type": "string", "description": "New file name without extension; renames the file. Single path only."},
            "waitSeconds": wait_property(DEFAULT_WAIT_SECS),
        }),
        &["paths"],
    )
}

fn resolve_schema() -> Value {
    object(
        json!({
            "paths": paths_property(MAX_PATHS),
            "keep": {"type": "string", "enum": ["app", "file"]},
        }),
        &["paths", "keep"],
    )
}

fn attribute_schema() -> Value {
    object(
        json!({
            "paths": paths_property(MAX_JOB_PATHS),
            "waitSeconds": wait_property(DEFAULT_WAIT_SECS),
        }),
        &["paths"],
    )
}

fn job_schema() -> Value {
    object(json!({"jobId": {"type": "string"}}), &["jobId"])
}

fn job_wait_schema() -> Value {
    object(
        json!({"jobId": {"type": "string"}, "waitSeconds": wait_property(30)}),
        &["jobId"],
    )
}

fn settings_schema() -> Value {
    object(
        json!({
            "values": {
                "type": "object",
                "description": "Setting keys mapped to their new values.",
                "additionalProperties": true,
            },
        }),
        &["values"],
    )
}

fn models_schema() -> Value {
    object(
        json!({"provider": {"type": "string", "enum": PROVIDER_IDS}}),
        &[],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_unique_and_valid() {
        let mut names: Vec<&str> = TOOLS.iter().map(|t| t.name).collect();
        assert!(names.iter().all(|n| !n.is_empty()
            && n.len() <= 128
            && n.chars()
                .all(|c| c.is_ascii_alphanumeric() || "_-.".contains(c))));
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), TOOLS.len());
    }

    #[test]
    fn every_schema_is_an_object_schema() {
        for tool in catalogue() {
            let schema = &tool["inputSchema"];
            assert_eq!(schema["type"], "object", "{}", tool["name"]);
            if let Some(required) = schema["required"].as_array() {
                for key in required {
                    assert!(
                        schema["properties"].get(key.as_str().unwrap()).is_some(),
                        "{}: {key}",
                        tool["name"]
                    );
                }
            }
        }
    }

    #[test]
    fn read_only_tools_are_not_destructive() {
        for t in TOOLS {
            assert!(!(t.read_only && t.destructive), "{}", t.name);
        }
        assert!(is_known("get_status"));
        assert!(!is_known("ai_set_api_key"));
    }
}
