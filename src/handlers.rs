use eframe::egui;
use crate::app::ImageApp;
use std::sync::atomic::Ordering;

fn next_scan_request_id(app: &ImageApp) -> u64 {
    app.state.scan_id.fetch_add(1, Ordering::AcqRel) + 1
}

pub fn request_directory_scan(app: &mut ImageApp, target_path: std::path::PathBuf) {
    let request_id = next_scan_request_id(app);
    let _ = app.state.dir_req_tx.send(crate::scanner::ScanRequest {
        target_path,
        sort_method: app.state.sort_method,
        sort_order: app.state.sort_order,
        request_id,
    });
}

/// Queues both image loading and directory scanning through the same runtime paths.
pub fn open_target(app: &mut ImageApp, path: std::path::PathBuf) {
    // Opening a new target starts a fresh context; no previous preload on first entry.
    app.state.preload.on_new_open();
    load_target_file(app, path.clone());
    request_directory_scan(app, path);
}

pub fn load_target_file(app: &mut ImageApp, path: std::path::PathBuf) {
    app.state.current_file_path = Some(path.clone());
    if let Some(name) = path.file_name() {
        app.state.current_file_name = name.to_string_lossy().into_owned();
        app.cached_title.clear();
    }
    app.state.current_file_size_bytes = std::fs::metadata(&path).ok().map(|m| m.len());

    reset_view_for_new_file(app);

    app.state.preload.process_worker_results();
    if let Some(cached) = app.state.preload.try_take_cached_for_path(&path) {
        // Invalidate any foreground decode and use cached payload on the next UI tick.
        let _ = app.state.load_id.fetch_add(1, Ordering::AcqRel);
        app.state.preload.set_instant_current(cached);
        return;
    }

    // Atomically increment ID to notify the background thread to abort current work.
    let current_id = app.state.load_id.fetch_add(1, Ordering::AcqRel) + 1;
    let _ = app.state.req_tx.send((path, current_id));
}

fn reset_view_for_new_file(app: &mut ImageApp) {
    app.state.frames.clear();
    app.state.frame_durations.clear();
    app.state.current_frame = 0;
    app.state.last_frame_time = None;
    app.state.image_resolution = None;
    app.state.load_error = None;
    app.state.auto_fit = true;
    app.state.pan = egui::Vec2::ZERO;
    app.state.target_scale = None;
    app.state.target_pan = None;
    app.state.reset_start_time = None;
    app.state.adjustments.reset_all();
    app.state.original_pixels.clear();
    app.state.adjustments_dirty = false;
    app.state.adjustments_last_changed = None;
}

fn apply_loaded_image(app: &mut ImageApp, ctx: &egui::Context, loaded_image: crate::image_io::LoadedImage) {
    app.state.frames.clear();
    app.state.frame_durations.clear();
    app.state.current_frame = 0;
    app.state.last_frame_time = None;
    app.state.image_resolution = Some((loaded_image.width, loaded_image.height));

    // Store original pixels for non-destructive adjustment recomputation
    app.state.original_pixels.clear();
    for frame in loaded_image.frames.iter() {
        app.state.original_pixels.push(frame.pixels.clone());
    }

    for (i, frame) in loaded_image.frames.iter().enumerate() {
        let color_image = egui::ColorImage::from_rgba_unmultiplied(
            [loaded_image.width as usize, loaded_image.height as usize],
            &frame.pixels,
        );
        app.state.frames.push(ctx.load_texture(
            format!("viewer_image_frame_{}", i),
            color_image,
            egui::TextureOptions::LINEAR,
        ));
        app.state.frame_durations.push(frame.duration_ms as f64 / 1000.0);
    }
    app.state.load_error = None;

    if let Some(path) = app.state.current_file_path.clone() {
        let index = app.state.current_index;
        let playlist_snapshot = app.state.playlist.clone();
        app.state
            .preload
            .on_current_image_ready(path, index, loaded_image, &playlist_snapshot, app.settings.loop_playlist);
    }
}

pub fn sync_window_state(app: &mut ImageApp, ctx: &egui::Context) {
    let is_maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
    if is_maximized != app.state.is_fullscreen {
        app.state.is_fullscreen = is_maximized;
    }
    if let Some(focused) = ctx.input(|i| i.viewport().focused) {
        if focused && !app.is_focused {
            let now = ctx.input(|i| i.time);
            app.focus_settle_until = now + 0.20;
        }
        app.is_focused = focused;
    }
}

pub fn process_image_loading(app: &mut ImageApp, ctx: &egui::Context) {
    app.state.preload.process_worker_results();

    if let Some(preloaded) = app.state.preload.take_instant_current() {
        apply_loaded_image(app, ctx, preloaded);
        return;
    }

    let mut latest_result = None;
    while let Ok(result) = app.state.res_rx.try_recv() {
        latest_result = Some(result);
    }

    if let Some(result) = latest_result {
        match result {
            Ok(loaded_image) => {
                // Only accept results for the latest active request.
                let expected_id = app.state.load_id.load(Ordering::Acquire);
                if loaded_image.request_id != expected_id {
                    return;
                }
                apply_loaded_image(app, ctx, loaded_image);
            }
            Err(load_failure) => {
                // Prevent stale error flashes from outdated decode jobs.
                let expected_id = app.state.load_id.load(Ordering::Acquire);
                if load_failure.request_id != expected_id {
                    return;
                }
                app.state.frames.clear();
                app.state.image_resolution = None;
                app.state.load_error = Some(format!("Unsupported or invalid file:\n{}", load_failure.message));
            }
        }
    }
}

/// Core logic for moving through the folder's images.
pub fn navigate(app: &mut ImageApp, direction: i32) {
    if app.state.playlist.is_empty() { return; }

    let current_idx = app.state.current_index;
    let playlist_len = app.state.playlist.len();
    let mut navigate_to = None;

    if direction > 0 { // Move Forward
        if app.settings.loop_playlist {
            navigate_to = Some((current_idx + 1) % playlist_len);
        } else if current_idx + 1 < playlist_len {
            navigate_to = Some(current_idx + 1);
        }
    } else if direction < 0 { // Move Backward
        if app.settings.loop_playlist {
            if current_idx == 0 {
                navigate_to = Some(playlist_len - 1);
            } else {
                navigate_to = Some(current_idx - 1);
            }
        } else if current_idx > 0 {
            navigate_to = Some(current_idx - 1);
        }
    }

    if let Some(new_index) = navigate_to {
        app.state.preload.on_navigation_away(direction);
        app.state.current_index = new_index;
        let next_path = app.state.playlist[new_index].clone();
        load_target_file(app, next_path);
    }
}

pub fn handle_keyboard(app: &mut ImageApp, ctx: &egui::Context) {
    ctx.input(|i| {
        if i.key_pressed(egui::Key::ArrowRight) {
            navigate(app, 1);
        } else if i.key_pressed(egui::Key::ArrowLeft) {
            navigate(app, -1);
        }
        // --- Gamma Adjustment: Key 5 = decrease, Key 6 = increase ---
        if i.key_pressed(egui::Key::Num5) {
            let gamma = &mut app.state.adjustments.gamma;
            gamma.value = (gamma.value - crate::adjustments::gamma::GammaAdjustment::STEP)
                .clamp(
                    crate::adjustments::gamma::GammaAdjustment::MIN,
                    crate::adjustments::gamma::GammaAdjustment::MAX,
                );
            app.state.adjustments_dirty = true;
            app.state.adjustments_last_changed = Some(i.time);
        }
        if i.key_pressed(egui::Key::Num6) {
            let gamma = &mut app.state.adjustments.gamma;
            gamma.value = (gamma.value + crate::adjustments::gamma::GammaAdjustment::STEP)
                .clamp(
                    crate::adjustments::gamma::GammaAdjustment::MIN,
                    crate::adjustments::gamma::GammaAdjustment::MAX,
                );
            app.state.adjustments_dirty = true;
            app.state.adjustments_last_changed = Some(i.time);
        }
    });
}

/// Rebuilds GPU textures from original pixels with current adjustments applied.
/// Only runs when the `adjustments_dirty` flag is set, to avoid per-frame work.
pub fn rebuild_adjusted_textures(app: &mut ImageApp, _ctx: &egui::Context) {
    if !app.state.adjustments_dirty {
        return;
    }
    app.state.adjustments_dirty = false;

    let (width, height) = match app.state.image_resolution {
        Some(res) => res,
        None => return,
    };

    // Re-apply all adjustments to the stored original pixels and re-upload textures
    for (i, original) in app.state.original_pixels.iter().enumerate() {
        let adjusted = if app.state.adjustments.has_adjustments() {
            app.state.adjustments.apply_all(original)
        } else {
            original.clone()
        };

        let color_image = egui::ColorImage::from_rgba_unmultiplied(
            [width as usize, height as usize],
            &adjusted,
        );

        if let Some(tex) = app.state.frames.get_mut(i) {
            // Update existing texture in-place (avoids GPU handle churn)
            tex.set(color_image, egui::TextureOptions::LINEAR);
        }
    }
}

pub fn process_directory_scanning(app: &mut ImageApp) {
    let mut latest_scan = None;
    while let Ok(result) = app.state.dir_res_rx.try_recv() {
        latest_scan = Some(result);
    }

    if let Some(scan) = latest_scan {
        let expected_id = app.state.scan_id.load(Ordering::Acquire);
        if scan.request_id != expected_id {
            return;
        }
        app.state.current_folder = Some(scan.folder_path);
        app.state.playlist = scan.playlist;
        app.state.current_index = scan.current_index;

        let playlist_snapshot = app.state.playlist.clone();
        let current_path = app.state.current_file_path.clone();
        app.state.preload.on_playlist_updated(
            &playlist_snapshot,
            app.state.current_index,
            app.settings.loop_playlist,
            current_path.as_ref(),
        );
    }
}

pub fn handle_drag_and_drop(app: &mut ImageApp, ctx: &egui::Context) {
    ctx.input(|i| {
        if let Some(dropped_file) = i.raw.dropped_files.first() {
            if let Some(path) = &dropped_file.path {
                app.state.preload.on_new_open();
                let mut should_scan = false;
                
                if let Some(parent) = path.parent() {
                    if Some(parent.to_path_buf()) == app.state.current_folder {
                        if let Some(idx) = app.state.playlist.iter().position(|p| p == path) {
                            app.state.current_index = idx;
                        } else {
                            should_scan = true;
                        }
                    } else {
                        // --- THE FIX: PREVENT ASYNC RACE CONDITIONS ---
                        // Immediately invalidate the old folder's state
                        app.state.current_folder = Some(parent.to_path_buf());
                        app.state.playlist.clear(); 
                        app.state.current_index = 0;
                        should_scan = true;
                    }
                }

                load_target_file(app, path.clone());
                if should_scan {
                    request_directory_scan(app, path.clone());
                }
            }
        }
    });
}