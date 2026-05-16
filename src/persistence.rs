use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PersistedDirectorySortPreference {
    pub sort_method: crate::scanner::SortMethod,
    pub sort_order: crate::scanner::SortOrder,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PersistedAppState {
    pub immersive_maximized: bool,
    pub loop_playlist: bool,
    pub fit_all_images_to_window: bool,
    pub pixel_based_1_to_1: bool,
    #[serde(default = "default_thumbnail_width")]
    pub thumbnail_width: u32,
    #[serde(default)]
    pub directory_sort_preferences: HashMap<String, PersistedDirectorySortPreference>,
}

impl Default for PersistedAppState {
    fn default() -> Self {
        Self {
            immersive_maximized: true,
            loop_playlist: false,
            fit_all_images_to_window: true,
            pixel_based_1_to_1: false,
            thumbnail_width: default_thumbnail_width(),
            directory_sort_preferences: HashMap::new(),
        }
    }
}

fn default_thumbnail_width() -> u32 {
    160
}

pub fn directory_key(path: &Path) -> String {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(path))
            .unwrap_or_else(|_| path.to_path_buf())
    };

    let normalized = std::fs::canonicalize(&absolute).unwrap_or(absolute);
    let as_string = normalized.to_string_lossy().into_owned();

    if cfg!(windows) {
        as_string.to_ascii_lowercase()
    } else {
        as_string
    }
}

pub fn load_persisted_state() -> PersistedAppState {
    let Some(path) = settings_file_path() else {
        return PersistedAppState::default();
    };

    let Ok(raw) = std::fs::read_to_string(path) else {
        return PersistedAppState::default();
    };

    serde_json::from_str::<PersistedAppState>(&raw).unwrap_or_default()
}

pub fn save_persisted_state(state: &PersistedAppState) -> Result<(), String> {
    let path = settings_file_path().ok_or_else(|| "No writable settings path found".to_string())?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create settings directory: {}", e))?;
    }

    let json = serde_json::to_string_pretty(state)
        .map_err(|e| format!("Failed to serialize settings: {}", e))?;

    std::fs::write(&path, json).map_err(|e| format!("Failed to write settings file: {}", e))
}

fn settings_file_path() -> Option<PathBuf> {
    if cfg!(windows) {
        if let Ok(appdata) = std::env::var("APPDATA") {
            return Some(PathBuf::from(appdata).join("image_viewer").join("settings.json"));
        }
    }

    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        return Some(PathBuf::from(xdg).join("image_viewer").join("settings.json"));
    }

    if let Ok(home) = std::env::var("HOME") {
        return Some(
            PathBuf::from(home)
                .join(".config")
                .join("image_viewer")
                .join("settings.json"),
        );
    }

    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|dir| dir.join("image_viewer-settings.json")))
}
