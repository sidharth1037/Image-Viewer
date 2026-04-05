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

fn current_sort_target_path(app: &ImageApp) -> Option<std::path::PathBuf> {
    if !app.state.active_playlist.is_empty() {
        return Some(app.state.active_playlist[app.state.current_index].clone());
    }

    if let Some(folder) = &app.state.current_folder {
        if !app.state.current_file_name.is_empty() {
            return Some(folder.join(&app.state.current_file_name));
        }
    }

    None
}

pub fn rescan_current_sort(app: &mut ImageApp) {
    if let Some(path) = current_sort_target_path(app) {
        request_directory_scan(app, path);
    }
}

pub fn set_sort_order(app: &mut ImageApp, order: crate::scanner::SortOrder) {
    app.state.sort_order = order;
    rescan_current_sort(app);
}

pub fn open_filter_popup(app: &mut ImageApp) {
    app.show_filter_popup = true;
    app.filter_popup_focus_pending = true;
    app.filter_popup_just_opened = true;
}

pub fn close_filter_popup(app: &mut ImageApp) {
    app.show_filter_popup = false;
    app.filter_popup_focus_pending = false;
    app.filter_popup_just_opened = false;
}

pub fn toggle_filter_popup(app: &mut ImageApp) {
    if app.show_filter_popup {
        close_filter_popup(app);
    } else {
        open_filter_popup(app);
    }
}

pub fn set_text_filter(app: &mut ImageApp, text: String) {
    if app.state.filter.criteria.text == text {
        return;
    }

    app.state.filter.criteria.text = text;
    rebuild_active_playlist_and_reconcile_current(app);
}

fn sort_method_name(method: crate::scanner::SortMethod) -> &'static str {
    match method {
        crate::scanner::SortMethod::Alphabetical => "Alphabetical",
        crate::scanner::SortMethod::Natural => "Natural",
        crate::scanner::SortMethod::Size => "Size",
        crate::scanner::SortMethod::DateModified => "Date modified",
        crate::scanner::SortMethod::DateCreated => "Date created",
    }
}

pub fn cycle_sort_method(app: &mut ImageApp, direction: i32) {
    use crate::scanner::SortMethod;

    let ordered = [
        SortMethod::Alphabetical,
        SortMethod::Natural,
        SortMethod::Size,
        SortMethod::DateModified,
        SortMethod::DateCreated,
    ];

    let current = ordered
        .iter()
        .position(|m| *m == app.state.sort_method)
        .unwrap_or(0);

    let next = if direction >= 0 {
        (current + 1) % ordered.len()
    } else if current == 0 {
        ordered.len() - 1
    } else {
        current - 1
    };

    app.state.sort_method = ordered[next];
    app.state.sort_order = crate::scanner::default_order_for(app.state.sort_method);
    rescan_current_sort(app);
}

pub fn jump_to_playlist_edge(app: &mut ImageApp, to_last: bool) {
    if app.state.active_playlist.is_empty() {
        return;
    }

    let target_index = if to_last {
        app.state.active_playlist.len() - 1
    } else {
        0
    };

    if app.state.current_index == target_index {
        return;
    }

    let direction = if to_last { 1 } else { -1 };
    app.state.preload.on_navigation_away(direction);
    app.state.current_index = target_index;
    let target_path = app.state.active_playlist[target_index].clone();
    load_target_file(app, target_path);
}

pub fn toggle_settings_window(app: &mut ImageApp) {
    app.show_settings_window = !app.show_settings_window;
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
    app.state.overlay_last_changed = None;
    app.state.overlay_text = None;
    app.state.show_original_while_held = false;
}

fn clear_current_view_for_empty_playlist(app: &mut ImageApp) {
    app.state.current_file_path = None;
    app.state.current_file_name.clear();
    app.state.current_file_size_bytes = None;
    app.state.frames.clear();
    app.state.frame_durations.clear();
    app.state.current_frame = 0;
    app.state.last_frame_time = None;
    app.state.image_resolution = None;
    app.state.load_error = None;
    app.state.original_pixels.clear();
    app.state.adjustments_dirty = false;
}

fn rebuild_active_playlist_and_reconcile_current(app: &mut ImageApp) {
    let previous_path = app.state.current_file_path.clone();
    app.state.active_playlist = crate::playlist_view::build_active_playlist(
        &app.state.source_playlist,
        &app.state.filter.criteria,
    );

    if app.state.active_playlist.is_empty() {
        app.state.current_index = 0;
        clear_current_view_for_empty_playlist(app);

        let playlist_snapshot = app.state.active_playlist.clone();
        app.state.preload.on_playlist_updated(
            &playlist_snapshot,
            app.state.current_index,
            app.settings.loop_playlist,
            None,
        );
        return;
    }

    let target_index = previous_path
        .as_ref()
        .and_then(|path| app.state.active_playlist.iter().position(|p| p == path))
        .unwrap_or(0);
    let target_path = app.state.active_playlist[target_index].clone();
    let changed_target = previous_path.as_ref() != Some(&target_path);

    app.state.current_index = target_index;
    if changed_target {
        load_target_file(app, target_path);
    }

    let playlist_snapshot = app.state.active_playlist.clone();
    let current_path = app.state.current_file_path.clone();
    app.state.preload.on_playlist_updated(
        &playlist_snapshot,
        app.state.current_index,
        app.settings.loop_playlist,
        current_path.as_ref(),
    );
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
        let playlist_snapshot = app.state.active_playlist.clone();
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
            app.focus_settle_until = now + 0.35;
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
    if app.state.active_playlist.is_empty() { return; }

    let current_idx = app.state.current_index;
    let playlist_len = app.state.active_playlist.len();
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
        let next_path = app.state.active_playlist[new_index].clone();
        load_target_file(app, next_path);
    }
}

pub fn handle_keyboard(app: &mut ImageApp, ctx: &egui::Context) {
    let shortcuts = app.settings.shortcuts;
    let input = ctx.input(|i| {
        (
            i.time,
            shortcuts.navigate_next.is_pressed(i),
            shortcuts.navigate_prev.is_pressed(i),
            shortcuts.jump_to_start.is_pressed(i),
            shortcuts.jump_to_end.is_pressed(i),
            shortcuts.cycle_sort_method_prev.is_pressed(i),
            shortcuts.cycle_sort_method_next.is_pressed(i),
            shortcuts.sort_ascending.is_pressed(i),
            shortcuts.sort_descending.is_pressed(i),
            shortcuts.toggle_settings.is_pressed(i),
            shortcuts.toggle_search.is_pressed(i),
            shortcuts.close_window.is_pressed(i),
            shortcuts.saturation_decrease.pressed_step_multiplier(i),
            shortcuts.saturation_increase.pressed_step_multiplier(i),
            shortcuts.contrast_decrease.pressed_step_multiplier(i),
            shortcuts.contrast_increase.pressed_step_multiplier(i),
            shortcuts.gamma_decrease.pressed_step_multiplier(i),
            shortcuts.gamma_increase.pressed_step_multiplier(i),
            shortcuts.exposure_decrease.pressed_step_multiplier(i),
            shortcuts.exposure_increase.pressed_step_multiplier(i),
            shortcuts.highlights_decrease.pressed_step_multiplier(i),
            shortcuts.highlights_increase.pressed_step_multiplier(i),
            shortcuts.shadows_decrease.pressed_step_multiplier(i),
            shortcuts.shadows_increase.pressed_step_multiplier(i),
            shortcuts.reset_adjustments.is_pressed(i),
            shortcuts.show_original_hold.is_held(i),
        )
    });

    let (
        time,
        go_next,
        go_prev,
        jump_start,
        jump_end,
        cycle_sort_prev,
        cycle_sort_next,
        sort_ascending,
        sort_descending,
        toggle_settings,
        toggle_search,
        close_window,
        saturation_down,
        saturation_up,
        contrast_down,
        contrast_up,
        gamma_down,
        gamma_up,
        exposure_down,
        exposure_up,
        highlights_down,
        highlights_up,
        shadows_down,
        shadows_up,
        reset_all,
        show_original_hold,
    ) = input;

    if app.show_filter_popup {
        if toggle_search {
            toggle_filter_popup(app);
            set_overlay_message(app, time, "Shortcut: Toggle filter popup");
        }
        return;
    }

    if go_next {
        navigate(app, 1);
        // set_overlay_message(app, time, "Shortcut: Next image");
    } else if go_prev {
        navigate(app, -1);
        // set_overlay_message(app, time, "Shortcut: Previous image");
    }

    if jump_start {
        jump_to_playlist_edge(app, false);
        set_overlay_message(app, time, "Shortcut: Jump to start");
    }
    if jump_end {
        jump_to_playlist_edge(app, true);
        set_overlay_message(app, time, "Shortcut: Jump to end");
    }

    if cycle_sort_prev {
        cycle_sort_method(app, -1);
        let text = format!("Shortcut: Sort type -> {}", sort_method_name(app.state.sort_method));
        set_overlay_message(app, time, &text);
    }
    if cycle_sort_next {
        cycle_sort_method(app, 1);
        let text = format!("Shortcut: Sort type -> {}", sort_method_name(app.state.sort_method));
        set_overlay_message(app, time, &text);
    }

    if sort_ascending {
        set_sort_order(app, crate::scanner::SortOrder::Ascending);
        set_overlay_message(app, time, "Shortcut: Sort ascending");
    }
    if sort_descending {
        set_sort_order(app, crate::scanner::SortOrder::Descending);
        set_overlay_message(app, time, "Shortcut: Sort descending");
    }

    if toggle_settings {
        toggle_settings_window(app);
        set_overlay_message(app, time, "Shortcut: Toggle settings");
    }

    if toggle_search {
        toggle_filter_popup(app);
        set_overlay_message(app, time, "Shortcut: Toggle filter popup");
    }

    if close_window {
        set_overlay_message(app, time, "Shortcut: Close window");
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }

    if let Some(multiplier) = saturation_down {
        let changed = {
            let saturation = &mut app.state.adjustments.saturation;
            saturation.adjust_by(-crate::adjustments::saturation::SaturationAdjustment::STEP * multiplier)
        };
        show_adjustment_overlay(app, time, crate::adjustments::pipeline::AdjustmentTarget::Saturation);
        mark_adjustments_changed(
            app,
            changed,
        );
    }
    if let Some(multiplier) = saturation_up {
        let changed = {
            let saturation = &mut app.state.adjustments.saturation;
            saturation.adjust_by(crate::adjustments::saturation::SaturationAdjustment::STEP * multiplier)
        };
        show_adjustment_overlay(app, time, crate::adjustments::pipeline::AdjustmentTarget::Saturation);
        mark_adjustments_changed(
            app,
            changed,
        );
    }

    if let Some(multiplier) = contrast_down {
        let changed = {
            let contrast = &mut app.state.adjustments.contrast;
            contrast.adjust_by(-crate::adjustments::contrast::ContrastAdjustment::STEP * multiplier)
        };
        show_adjustment_overlay(app, time, crate::adjustments::pipeline::AdjustmentTarget::Contrast);
        mark_adjustments_changed(
            app,
            changed,
        );
    }
    if let Some(multiplier) = contrast_up {
        let changed = {
            let contrast = &mut app.state.adjustments.contrast;
            contrast.adjust_by(crate::adjustments::contrast::ContrastAdjustment::STEP * multiplier)
        };
        show_adjustment_overlay(app, time, crate::adjustments::pipeline::AdjustmentTarget::Contrast);
        mark_adjustments_changed(
            app,
            changed,
        );
    }

    if let Some(multiplier) = gamma_down {
        let changed = {
            let gamma = &mut app.state.adjustments.gamma;
            gamma.adjust_by(-crate::adjustments::gamma::GammaAdjustment::STEP * multiplier)
        };
        show_adjustment_overlay(app, time, crate::adjustments::pipeline::AdjustmentTarget::Gamma);
        mark_adjustments_changed(
            app,
            changed,
        );
    }
    if let Some(multiplier) = gamma_up {
        let changed = {
            let gamma = &mut app.state.adjustments.gamma;
            gamma.adjust_by(crate::adjustments::gamma::GammaAdjustment::STEP * multiplier)
        };
        show_adjustment_overlay(app, time, crate::adjustments::pipeline::AdjustmentTarget::Gamma);
        mark_adjustments_changed(
            app,
            changed,
        );
    }

    if let Some(multiplier) = exposure_down {
        let changed = {
            let exposure = &mut app.state.adjustments.exposure;
            exposure.adjust_by(-crate::adjustments::exposure::ExposureAdjustment::STEP * multiplier)
        };
        show_adjustment_overlay(app, time, crate::adjustments::pipeline::AdjustmentTarget::Exposure);
        mark_adjustments_changed(
            app,
            changed,
        );
    }
    if let Some(multiplier) = exposure_up {
        let changed = {
            let exposure = &mut app.state.adjustments.exposure;
            exposure.adjust_by(crate::adjustments::exposure::ExposureAdjustment::STEP * multiplier)
        };
        show_adjustment_overlay(app, time, crate::adjustments::pipeline::AdjustmentTarget::Exposure);
        mark_adjustments_changed(
            app,
            changed,
        );
    }

    if let Some(multiplier) = highlights_down {
        let changed = {
            let highlights = &mut app.state.adjustments.highlights;
            highlights.adjust_by(-crate::adjustments::highlights::HighlightsAdjustment::STEP * multiplier)
        };
        show_adjustment_overlay(app, time, crate::adjustments::pipeline::AdjustmentTarget::Highlights);
        mark_adjustments_changed(
            app,
            changed,
        );
    }
    if let Some(multiplier) = highlights_up {
        let changed = {
            let highlights = &mut app.state.adjustments.highlights;
            highlights.adjust_by(crate::adjustments::highlights::HighlightsAdjustment::STEP * multiplier)
        };
        show_adjustment_overlay(app, time, crate::adjustments::pipeline::AdjustmentTarget::Highlights);
        mark_adjustments_changed(
            app,
            changed,
        );
    }

    if let Some(multiplier) = shadows_down {
        let changed = {
            let shadows = &mut app.state.adjustments.shadows;
            shadows.adjust_by(-crate::adjustments::shadows::ShadowsAdjustment::STEP * multiplier)
        };
        show_adjustment_overlay(app, time, crate::adjustments::pipeline::AdjustmentTarget::Shadows);
        mark_adjustments_changed(
            app,
            changed,
        );
    }
    if let Some(multiplier) = shadows_up {
        let changed = {
            let shadows = &mut app.state.adjustments.shadows;
            shadows.adjust_by(crate::adjustments::shadows::ShadowsAdjustment::STEP * multiplier)
        };
        show_adjustment_overlay(app, time, crate::adjustments::pipeline::AdjustmentTarget::Shadows);
        mark_adjustments_changed(
            app,
            changed,
        );
    }

    if reset_all {
        let had_changes = app.state.adjustments.has_adjustments();
        app.state.adjustments.reset_all();
        set_overlay_message(app, time, "Shortcut: Reset adjustments");
        if had_changes {
            app.state.adjustments_dirty = true;
        }
    }

    if app.state.show_original_while_held != show_original_hold {
        app.state.show_original_while_held = show_original_hold;
        app.state.adjustments_dirty = true;
        if show_original_hold {
            set_overlay_message(app, time, "Shortcut: Show original");
        }
    }
}

fn mark_adjustments_changed(app: &mut ImageApp, changed: bool) {
    if changed {
        app.state.adjustments_dirty = true;
    }
}

fn show_adjustment_overlay(
    app: &mut ImageApp,
    time: f64,
    target: crate::adjustments::pipeline::AdjustmentTarget,
) {
    let message = app.state.adjustments.overlay_text_for(target);
    set_overlay_message(app, time, &message);
}

fn set_overlay_message(app: &mut ImageApp, time: f64, text: &str) {
    app.state.overlay_last_changed = Some(time);
    app.state.overlay_text = Some(text.to_string());
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
        let adjusted = if app.state.show_original_while_held {
            original.clone()
        } else if app.state.adjustments.has_adjustments() {
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
        app.state.source_playlist = scan.playlist;
        rebuild_active_playlist_and_reconcile_current(app);
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
                        if let Some(idx) = app.state.active_playlist.iter().position(|p| p == path) {
                            app.state.current_index = idx;
                        } else {
                            should_scan = true;
                        }
                    } else {
                        // --- THE FIX: PREVENT ASYNC RACE CONDITIONS ---
                        // Immediately invalidate the old folder's state
                        app.state.current_folder = Some(parent.to_path_buf());
                        app.state.source_playlist.clear();
                        app.state.active_playlist.clear();
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