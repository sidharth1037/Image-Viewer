use crate::app::ImageApp;
use crate::state::ViewerState;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum DeleteCurrentFileOutcome {
    Deleted {
        deleted_path: PathBuf,
    },
    NoFileToDelete,
    Failed {
        message: String,
    },
}

#[derive(Clone)]
struct ViewSnapshot {
    current_path: Option<PathBuf>,
    current_index: usize,
    active_playlist: Vec<PathBuf>,
}

pub fn delete_file_permanently(
    app: &mut ImageApp,
    requested_path: Option<PathBuf>,
) -> DeleteCurrentFileOutcome {
    let active_index = app.workspace.active_view_index;
    let snapshots: Vec<ViewSnapshot> = app
        .workspace
        .views
        .iter()
        .map(snapshot_view)
        .collect();

    let deleted_path = requested_path
        .or_else(|| snapshots[active_index].current_path.clone())
        .or_else(|| app.workspace.views[active_index].current_file_path.clone());

    let Some(deleted_path) = deleted_path else {
        return DeleteCurrentFileOutcome::NoFileToDelete;
    };

    let active_snapshot = &snapshots[active_index];
    let active_preferred_path = neighbor_after_delete(
        &active_snapshot.active_playlist,
        active_snapshot.current_index,
        &deleted_path,
    );

    if let Err(error) = std::fs::remove_file(&deleted_path) {
        return DeleteCurrentFileOutcome::Failed {
            message: error.to_string(),
        };
    }

    let mut reload_requests = Vec::new();

    for (index, view) in app.workspace.views.iter_mut().enumerate() {
        let snapshot = &snapshots[index];
        let preferred_path = if index == active_index {
            active_preferred_path.clone()
        } else if snapshot.current_path.as_ref() == Some(&deleted_path) {
            neighbor_after_delete(&snapshot.active_playlist, snapshot.current_index, &deleted_path)
        } else {
            snapshot.current_path.clone()
        };

        view.source_playlist.retain(|path| path != &deleted_path);
        view.active_playlist = crate::playlist_view::build_active_playlist(
            &view.source_playlist,
            &view.filter.criteria,
        );

        if view.active_playlist.is_empty() {
            clear_view_for_empty_playlist(view);
            let playlist_snapshot = view.active_playlist.clone();
            let current_index = view.current_index;
            view.preload.on_playlist_updated(
                &playlist_snapshot,
                current_index,
                app.settings.loop_playlist,
                None,
            );
            continue;
        }

        let resolved_path = resolve_current_path(
            &view.active_playlist,
            preferred_path,
            snapshot.current_index,
        );

        let Some(path) = resolved_path else {
            clear_view_for_empty_playlist(view);
            let playlist_snapshot = view.active_playlist.clone();
            let current_index = view.current_index;
            view.preload.on_playlist_updated(
                &playlist_snapshot,
                current_index,
                app.settings.loop_playlist,
                None,
            );
            continue;
        };

        let resolved_index = view
            .active_playlist
            .iter()
            .position(|candidate| candidate == &path)
            .unwrap_or(0);

        view.current_index = resolved_index;
        view.current_file_path = Some(path.clone());
        view.current_file_name = file_name_of(&path);
        view.current_file_size_bytes = std::fs::metadata(&path).ok().map(|metadata| metadata.len());
        view.load_error = None;

        let current_path = view.current_file_path.clone();
        let playlist_snapshot = view.active_playlist.clone();
        let current_index = view.current_index;
        view.preload.on_playlist_updated(
            &playlist_snapshot,
            current_index,
            app.settings.loop_playlist,
            current_path.as_ref(),
        );

        let should_reload = index == active_index || snapshot.current_path.as_ref() != Some(&path);
        if should_reload {
            reload_requests.push((index, path));
        }
    }

    let original_active_index = app.workspace.active_view_index;
    for (index, path) in reload_requests {
        app.workspace.active_view_index = index;
        crate::handlers::load_target_file(app, path);
    }

    app.workspace.active_view_index = active_index;
    if let Some(target_path) = app.workspace.views[active_index].current_file_path.clone() {
        crate::handlers::request_directory_scan(app, target_path);
    } else if deleted_path.parent().is_some() {
        crate::handlers::request_directory_scan(app, deleted_path.clone());
    }

    app.workspace.active_view_index = original_active_index;

    DeleteCurrentFileOutcome::Deleted { deleted_path }
}

fn snapshot_view(view: &ViewerState) -> ViewSnapshot {
    ViewSnapshot {
        current_path: view.current_file_path.clone(),
        current_index: view.current_index,
        active_playlist: view.active_playlist.clone(),
    }
}

fn neighbor_after_delete(
    playlist: &[PathBuf],
    current_index: usize,
    deleted_path: &Path,
) -> Option<PathBuf> {
    if playlist.len() <= 1 {
        return None;
    }

    let deleted_index = playlist
        .iter()
        .position(|path| path.as_path() == deleted_path)
        .unwrap_or_else(|| current_index.min(playlist.len() - 1));

    if deleted_index + 1 < playlist.len() {
        Some(playlist[deleted_index + 1].clone())
    } else {
        Some(playlist[deleted_index - 1].clone())
    }
}

fn resolve_current_path(
    active_playlist: &[PathBuf],
    preferred_path: Option<PathBuf>,
    fallback_index: usize,
) -> Option<PathBuf> {
    if active_playlist.is_empty() {
        return None;
    }

    if let Some(path) = preferred_path {
        if active_playlist.iter().any(|candidate| candidate == &path) {
            return Some(path);
        }
    }

    let index = fallback_index.min(active_playlist.len() - 1);
    Some(active_playlist[index].clone())
}

fn clear_view_for_empty_playlist(view: &mut ViewerState) {
    view.current_index = 0;
    view.current_file_path = None;
    view.current_file_name.clear();
    view.current_file_size_bytes = None;
    view.frames.clear();
    view.frame_durations.clear();
    view.current_frame = 0;
    view.last_frame_time = None;
    view.image_resolution = None;
    view.image_density = None;
    view.load_error = None;
    view.auto_fit = true;
    view.pan = eframe::egui::Vec2::ZERO;
    view.target_scale = None;
    view.target_pan = None;
    view.reset_start_time = None;
    view.adjustments.reset_all();
    view.original_pixels.clear();
    view.adjustments_dirty = false;
    view.rotation_quarter_turns = 0;
    view.overlay_last_changed = None;
    view.overlay_text = None;
    view.show_original_while_held = false;
}

fn file_name_of(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}
