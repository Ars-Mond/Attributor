use serde::Serialize;
use tauri::{AppHandle, Emitter};

#[derive(Serialize, Clone)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum AppEvent {
    ThumbnailReady {
        path: String,
        thumb_360: String,
        thumb_1920: String,
    },
}

impl AppEvent {
    fn name(&self) -> &'static str {
        match self {
            AppEvent::ThumbnailReady { .. } => "thumbnail-ready",
        }
    }
}

pub fn invoke(app: &AppHandle, event: AppEvent) {
    let name = event.name();
    if let Err(e) = app.emit(name, &event) {
        log::warn!("Failed to emit {name}: {e}");
    }
}
