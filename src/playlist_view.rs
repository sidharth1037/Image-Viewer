use std::path::PathBuf;

use crate::state::FilterCriteria;

pub fn build_active_playlist(source_playlist: &[PathBuf], criteria: &FilterCriteria) -> Vec<PathBuf> {
    let needle = criteria.text.trim().to_lowercase();
    if needle.is_empty() {
        return source_playlist.to_vec();
    }

    source_playlist
        .iter()
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .map(|name| name.to_lowercase().contains(&needle))
                .unwrap_or(false)
        })
        .cloned()
        .collect()
}
