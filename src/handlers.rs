use eframe::egui;
use crate::app::ImageApp;
use crate::ui::dialogs::confirmation_dialog::ConfirmationSelection;
use std::sync::atomic::Ordering;

#[cfg(windows)]
#[link(name = "ole32")]
unsafe extern "system" {
    fn CoInitializeEx(pv_reserved: *mut core::ffi::c_void, coinit: u32) -> i32;
    fn CoUninitialize();
    fn CoTaskMemFree(pv: *const core::ffi::c_void);
}

#[cfg(windows)]
#[link(name = "shell32")]
unsafe extern "system" {
    fn SHParseDisplayName(
        psz_name: *const u16,
        pbc: *mut core::ffi::c_void,
        ppidl: *mut *mut core::ffi::c_void,
        sfgao_in: u32,
        psfgao_out: *mut u32,
    ) -> i32;

    fn SHOpenFolderAndSelectItems(
        pidl_folder: *const core::ffi::c_void,
        cidl: u32,
        apidl: *const *const core::ffi::c_void,
        dw_flags: u32,
    ) -> i32;
}

#[cfg(windows)]
const COINIT_APARTMENTTHREADED: u32 = 0x2;
#[cfg(windows)]
const RPC_E_CHANGED_MODE: i32 = 0x80010106u32 as i32;

fn next_scan_request_id(app: &ImageApp) -> u64 {
    app.workspace.active_view().scan_id.fetch_add(1, Ordering::AcqRel) + 1
}

fn persist_sort_preference_for_directory(app: &mut ImageApp, directory: &std::path::Path) {
    let key = crate::persistence::directory_key(directory);
    let default_method = crate::scanner::SortMethod::Natural;
    let default_order = crate::scanner::default_order_for(default_method);
    let is_default = app.workspace.active_view().sort_method == default_method && app.workspace.active_view().sort_order == default_order;

    if is_default {
        app.settings.directory_sort_preferences.remove(&key);
        return;
    }

    app.settings.directory_sort_preferences.insert(
        key,
        crate::persistence::PersistedDirectorySortPreference {
            sort_method: app.workspace.active_view().sort_method,
            sort_order: app.workspace.active_view().sort_order,
        },
    );
}

fn apply_sort_preference_for_directory(app: &mut ImageApp, directory: &std::path::Path) {
    let key = crate::persistence::directory_key(directory);
    let default_method = crate::scanner::SortMethod::Natural;
    let default_order = crate::scanner::default_order_for(default_method);

    if let Some(pref) = app.settings.directory_sort_preferences.get(&key) {
        app.workspace.active_view_mut().sort_method = pref.sort_method;
        app.workspace.active_view_mut().sort_order = pref.sort_order;
    } else {
        app.workspace.active_view_mut().sort_method = default_method;
        app.workspace.active_view_mut().sort_order = default_order;
    }
}

pub fn request_directory_scan(app: &mut ImageApp, target_path: std::path::PathBuf) {
    if let Some(target_dir) = target_path.parent() {
        if app
            .workspace.active_view()
            .current_folder
            .as_ref()
            .is_some_and(|current| current.as_path() == target_dir)
        {
            persist_sort_preference_for_directory(app, target_dir);
        } else {
            apply_sort_preference_for_directory(app, target_dir);
        }
    }

    let request_id = next_scan_request_id(app);
    let _ = app.workspace.active_view().dir_req_tx.send(crate::scanner::ScanRequest {
        target_path,
        sort_method: app.workspace.active_view().sort_method,
        sort_order: app.workspace.active_view().sort_order,
        request_id,
    });
}

fn current_sort_target_path(app: &ImageApp) -> Option<std::path::PathBuf> {
    if !app.workspace.active_view().active_playlist.is_empty() {
        return Some(app.workspace.active_view().active_playlist[app.workspace.active_view().current_index].clone());
    }

    if let Some(folder) = &app.workspace.active_view().current_folder {
        if !app.workspace.active_view().current_file_name.is_empty() {
            return Some(folder.join(&app.workspace.active_view().current_file_name));
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
    app.workspace.active_view_mut().sort_order = order;
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

pub fn open_delete_file_dialog(app: &mut ImageApp, time: f64) {
    let Some(path) = app.workspace.active_view().current_file_path.clone() else {
        set_overlay_message(app, time, "Shortcut: No file to delete");
        return;
    };

    close_filter_popup(app);
    app.show_sort_menu = false;
    app.sort_menu_pos = None;

    app.show_delete_file_dialog = true;
    app.delete_file_dialog_target = Some(path);
    app.delete_file_dialog_selection = ConfirmationSelection::Confirm;
}

pub fn cancel_delete_file_dialog(app: &mut ImageApp) {
    app.show_delete_file_dialog = false;
    app.delete_file_dialog_target = None;
    app.delete_file_dialog_selection = ConfirmationSelection::Confirm;
}

pub fn confirm_delete_file_dialog(app: &mut ImageApp, time: f64) {
    let requested_path = app.delete_file_dialog_target.clone();
    let outcome = crate::file_ops::delete_current::delete_file_permanently(app, requested_path);
    cancel_delete_file_dialog(app);

    match outcome {
        crate::file_ops::delete_current::DeleteCurrentFileOutcome::Deleted { deleted_path } => {
            let name = deleted_path
                .file_name()
                .map(|value| value.to_string_lossy().into_owned())
                .unwrap_or_else(|| deleted_path.to_string_lossy().into_owned());
            let text = format!("Shortcut: Deleted {}", name);
            set_overlay_message(app, time, &text);
        }
        crate::file_ops::delete_current::DeleteCurrentFileOutcome::NoFileToDelete => {
            set_overlay_message(app, time, "Shortcut: No file to delete");
        }
        crate::file_ops::delete_current::DeleteCurrentFileOutcome::Failed { message } => {
            let text = format!("Shortcut: Delete failed ({})", message);
            set_overlay_message(app, time, &text);
        }
    }
}

pub fn open_save_overwrite_dialog(app: &mut ImageApp, time: f64) {
    if app.workspace.active_view().current_file_path.is_none() {
        set_overlay_message(app, time, "Shortcut: No file to save");
        return;
    }

    if !has_overwritable_adjustment_changes(app) {
        set_overlay_message(app, time, "Shortcut: No adjustments to save");
        return;
    }

    if app.workspace.active_view().original_pixels.is_empty() || app.workspace.active_view().image_resolution.is_none() {
        set_overlay_message(app, time, "Shortcut: Image is not ready to save");
        return;
    }

    close_filter_popup(app);
    app.show_sort_menu = false;
    app.sort_menu_pos = None;
    app.show_save_overwrite_dialog = true;
}

pub fn cancel_save_overwrite_dialog(app: &mut ImageApp) {
    app.show_save_overwrite_dialog = false;
}

pub fn confirm_save_overwrite_dialog(app: &mut ImageApp, time: f64) {
    app.show_save_overwrite_dialog = false;
    overwrite_current_file_with_adjustments(app, time);
}

fn handle_delete_file_dialog_keyboard(app: &mut ImageApp, ctx: &egui::Context) {
    let input = ctx.input(|i| {
        (
            i.time,
            i.key_pressed(egui::Key::ArrowLeft),
            i.key_pressed(egui::Key::ArrowRight),
            i.key_pressed(egui::Key::ArrowUp),
            i.key_pressed(egui::Key::ArrowDown),
            i.key_pressed(egui::Key::Enter),
            i.key_pressed(egui::Key::Escape),
        )
    });

    let (time, arrow_left, arrow_right, arrow_up, arrow_down, enter, escape) = input;

    if arrow_left || arrow_up {
        app.delete_file_dialog_selection = ConfirmationSelection::Cancel;
    }

    if arrow_right || arrow_down {
        app.delete_file_dialog_selection = ConfirmationSelection::Confirm;
    }

    if escape {
        cancel_delete_file_dialog(app);
        return;
    }

    if enter {
        match app.delete_file_dialog_selection {
            ConfirmationSelection::Cancel => cancel_delete_file_dialog(app),
            ConfirmationSelection::Confirm => confirm_delete_file_dialog(app, time),
        }
    }
}

fn delete_file_shortcut_pressed(
    input: &egui::InputState,
    shortcut: crate::shortcuts::Shortcut,
) -> bool {
    if shortcut.is_pressed(input) {
        return true;
    }

    #[cfg(windows)]
    {
        // egui-winit maps Shift+Delete to Event::Cut on Windows, so a Key::Delete
        // press may never reach InputState::key_pressed(Key::Delete).
        let modifiers = input.modifiers;
        let looks_like_shift_delete = modifiers.shift
            && !modifiers.alt
            && !modifiers.ctrl
            && !modifiers.command;

        if looks_like_shift_delete && input.events.iter().any(|event| matches!(event, egui::Event::Cut)) {
            return true;
        }
    }

    false
}

pub fn set_text_filter(app: &mut ImageApp, text: String) {
    if app.workspace.active_view_mut().filter.criteria.text == text {
        return;
    }

    app.workspace.active_view_mut().filter.criteria.text = text;
    let index = app.workspace.active_view_index;
    rebuild_active_playlist_and_reconcile_current(app, index);
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
        .position(|m| *m == app.workspace.active_view().sort_method)
        .unwrap_or(0);

    let next = if direction >= 0 {
        (current + 1) % ordered.len()
    } else if current == 0 {
        ordered.len() - 1
    } else {
        current - 1
    };

    app.workspace.active_view_mut().sort_method = ordered[next];
    app.workspace.active_view_mut().sort_order = crate::scanner::default_order_for(app.workspace.active_view().sort_method);
    rescan_current_sort(app);
}

pub fn jump_to_playlist_edge(app: &mut ImageApp, to_last: bool) {
    if app.workspace.active_view().active_playlist.is_empty() {
        return;
    }

    let target_index = if to_last {
        app.workspace.active_view().active_playlist.len() - 1
    } else {
        0
    };

    if app.workspace.active_view().current_index == target_index {
        return;
    }

    let direction = if to_last { 1 } else { -1 };
    app.workspace.active_view_mut().preload.on_navigation_away(direction);
    app.workspace.active_view_mut().current_index = target_index;
    let target_path = app.workspace.active_view().active_playlist[target_index].clone();
    load_target_file(app, target_path);
}

pub fn toggle_settings_window(app: &mut ImageApp) {
    app.show_settings_window = !app.show_settings_window;
}

/// Queues both image loading and directory scanning through the same runtime paths.
pub fn open_target(app: &mut ImageApp, path: std::path::PathBuf) {
    // Opening a new target starts a fresh context; no previous preload on first entry.
    app.workspace.active_view_mut().preload.on_new_open();
    load_target_file(app, path.clone());
    request_directory_scan(app, path);
}

pub fn load_target_file(app: &mut ImageApp, path: std::path::PathBuf) {
    app.workspace.active_view_mut().current_file_path = Some(path.clone());
    if let Some(name) = path.file_name() {
        app.workspace.active_view_mut().current_file_name = name.to_string_lossy().into_owned();
        app.cached_title.clear();
    }
    app.workspace.active_view_mut().current_file_size_bytes = std::fs::metadata(&path).ok().map(|m| m.len());

    reset_view_for_new_file(app);

    let active_view = app.workspace.active_view_mut();
    active_view.preload.process_worker_results();
    if let Some(cached) = active_view.preload.try_take_cached_for_path(&path) {
        // Invalidate any foreground decode and use cached payload on the next UI tick.
        let _ = active_view.load_id.fetch_add(1, Ordering::AcqRel);
        active_view.preload.set_instant_current(cached);
        return;
    }

    // Atomically increment ID to notify the background thread to abort current work.
    let current_id = active_view.load_id.fetch_add(1, Ordering::AcqRel) + 1;
    let _ = active_view.req_tx.send((path, current_id));
}

fn reset_view_for_new_file(app: &mut ImageApp) {
    let active_view = app.workspace.active_view_mut();
    active_view.frames.clear();
    active_view.frame_durations.clear();
    active_view.current_frame = 0;
    active_view.last_frame_time = None;
    active_view.image_resolution = None;
    active_view.image_density = None;
    active_view.load_error = None;
    active_view.auto_fit = true;
    active_view.pan = egui::Vec2::ZERO;
    active_view.target_scale = None;
    active_view.target_pan = None;
    active_view.reset_start_time = None;
    active_view.adjustments.reset_all();
    active_view.original_pixels.clear();
    active_view.adjustments_dirty = false;
    active_view.rotation_quarter_turns = 0;
    active_view.overlay_last_changed = None;
    active_view.overlay_text = None;
    active_view.show_original_while_held = false;
}

fn clear_current_view_for_empty_playlist(app: &mut ImageApp, index: usize) {
    let view = &mut app.workspace.views[index];
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
    view.original_pixels.clear();
    view.adjustments_dirty = false;
    view.rotation_quarter_turns = 0;
}

fn rebuild_active_playlist_and_reconcile_current(app: &mut ImageApp, index: usize) {
    let previous_path = app.workspace.views[index].current_file_path.clone();
    let criteria = app.workspace.views[index].filter.criteria.clone();
    app.workspace.views[index].active_playlist = crate::playlist_view::build_active_playlist(
        &app.workspace.views[index].source_playlist,
        &criteria,
    );

    if app.workspace.views[index].active_playlist.is_empty() {
        app.workspace.views[index].current_index = 0;
        clear_current_view_for_empty_playlist(app, index);

        let playlist_snapshot = app.workspace.views[index].active_playlist.clone();
        let current_index = app.workspace.views[index].current_index;
        app.workspace.views[index].preload.on_playlist_updated(
            &playlist_snapshot,
            current_index,
            app.settings.loop_playlist,
            None,
        );
        return;
    }

    let target_index = previous_path
        .as_ref()
        .and_then(|path| app.workspace.views[index].active_playlist.iter().position(|p| p == path))
        .unwrap_or(0);
    let target_path = app.workspace.views[index].active_playlist[target_index].clone();
    let changed_target = previous_path.as_ref() != Some(&target_path);

    app.workspace.views[index].current_index = target_index;
    if changed_target {
        // Force the file to load on active view?
        // Wait, `load_target_file` loads on active view only!
        // For simplicity let's save the current view, switch to index, load, switch back.
        let active = app.workspace.active_view_index;
        app.workspace.active_view_index = index;
        load_target_file(app, target_path);
        app.workspace.active_view_index = active;
    }

    let view = &mut app.workspace.views[index];
    let playlist_snapshot = view.active_playlist.clone();
    let current_path = view.current_file_path.clone();
    let current_index = view.current_index;
    let loop_playlist = app.settings.loop_playlist;
    
    view.preload.on_playlist_updated(
        &playlist_snapshot,
        current_index,
        loop_playlist,
        current_path.as_ref(),
    );
}

fn apply_loaded_image(app: &mut ImageApp, ctx: &egui::Context, loaded_image: crate::image_io::LoadedImage, view_index: usize) {
    let view = &mut app.workspace.views[view_index];
    view.frames.clear();
    view.frame_durations.clear();
    view.current_frame = 0;
    view.last_frame_time = None;
    view.image_resolution = Some((loaded_image.width, loaded_image.height));
    view.image_density = loaded_image.density;

    // Store original pixels for non-destructive adjustment recomputation
    view.original_pixels.clear();
    for frame in loaded_image.frames.iter() {
        view.original_pixels.push(frame.pixels.clone());
    }

    for (i, frame) in loaded_image.frames.iter().enumerate() {
        let color_image = egui::ColorImage::from_rgba_unmultiplied(
            [loaded_image.width as usize, loaded_image.height as usize],
            &frame.pixels,
        );
        view.frames.push(ctx.load_texture(
            format!("viewer_{}_image_frame_{}", view_index, i),
            color_image,
            egui::TextureOptions::LINEAR,
        ));
        view.frame_durations.push(frame.duration_ms as f64 / 1000.0);
    }
    view.load_error = None;

    if let Some(path) = view.current_file_path.clone() {
        let index = view.current_index;
        let playlist_snapshot = view.active_playlist.clone();
        view
            .preload
            .on_current_image_ready(path, index, loaded_image, &playlist_snapshot, app.settings.loop_playlist);
    }
}

pub fn sync_window_state(app: &mut ImageApp, ctx: &egui::Context) {
    if let Some(is_maximized) = ctx.input(|i| i.viewport().maximized) {
        for state in &mut app.workspace.views {
            if is_maximized != state.is_fullscreen {
                state.is_fullscreen = is_maximized;
            }
        }
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
    for i in 0..app.workspace.views.len() {
        app.workspace.views[i].preload.process_worker_results();

        if let Some(preloaded) = app.workspace.views[i].preload.take_instant_current() {
            apply_loaded_image(app, ctx, preloaded, i);
            continue;
        }

        let mut latest_result = None;
        while let Ok(result) = app.workspace.views[i].res_rx.try_recv() {
            latest_result = Some(result);
        }

        if let Some(result) = latest_result {
            match result {
                Ok(loaded_image) => {
                    let expected_id = app.workspace.views[i].load_id.load(Ordering::Acquire);
                    if loaded_image.request_id != expected_id {
                        continue;
                    }
                    apply_loaded_image(app, ctx, loaded_image, i);
                }
                Err(load_failure) => {
                    let expected_id = app.workspace.views[i].load_id.load(Ordering::Acquire);
                    if load_failure.request_id != expected_id {
                        continue;
                    }
                    app.workspace.views[i].frames.clear();
                    app.workspace.views[i].image_resolution = None;
                    app.workspace.views[i].image_density = None;
                    app.workspace.views[i].load_error = Some(format!("Unsupported or invalid file:\n{}", load_failure.message));
                }
            }
        }
    }
}

/// Core logic for moving through the folder's images.
pub fn navigate(app: &mut ImageApp, direction: i32) {
    if direction == 0 { return; }
    if app.workspace.active_view().active_playlist.is_empty() { return; }

    let current_idx = app.workspace.active_view().current_index;
    let playlist_len = app.workspace.active_view().active_playlist.len();
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
        app.workspace.active_view_mut().preload.on_navigation_away(direction);
        app.workspace.active_view_mut().current_index = new_index;
        let next_path = app.workspace.active_view().active_playlist[new_index].clone();
        load_target_file(app, next_path);
    }
}

pub fn jump_to_index(app: &mut ImageApp, one_based_index: usize) {
    if app.workspace.active_view().active_playlist.is_empty() {
        return;
    }

    let playlist_len = app.workspace.active_view().active_playlist.len();
    let target_index = one_based_index.saturating_sub(1).min(playlist_len - 1);
    if target_index == app.workspace.active_view().current_index {
        return;
    }

    let current_index = app.workspace.active_view().current_index;
    let direction = if target_index > current_index { 1 } else { -1 };
    app.workspace.active_view_mut().preload.on_navigation_away(direction);
    app.workspace.active_view_mut().current_index = target_index;
    let target_path = app.workspace.active_view().active_playlist[target_index].clone();
    load_target_file(app, target_path);
}

pub fn handle_keyboard(app: &mut ImageApp, ctx: &egui::Context) {
    if app.show_delete_file_dialog {
        handle_delete_file_dialog_keyboard(app, ctx);
        return;
    }

    if app.show_save_overwrite_dialog {
        return;
    }

    if ctx.wants_keyboard_input() {
        return;
    }

    let shortcuts = app.settings.shortcuts;
    let input = ctx.input(|i| {
        (
            i.time,
            i.modifiers.alt,
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
            shortcuts.toggle_toolbar.is_pressed(i),
            shortcuts.reveal_in_explorer.is_pressed(i),
            delete_file_shortcut_pressed(i, shortcuts.delete_current_file_permanently),
            shortcuts.overwrite_with_adjustments.is_pressed(i),
            shortcuts.reload_current_context.is_pressed(i),
            shortcuts.rotate_clockwise.is_pressed(i),
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
        alt_held,
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
        toggle_toolbar,
        reveal_in_explorer,
        delete_current_file_permanently,
        overwrite_with_adjustments,
        reload_current_context,
        rotate_clockwise,
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

    let is_split_toggle = ctx.input(|i| i.modifiers.alt && i.key_pressed(egui::Key::C));

    if is_split_toggle {
        app.workspace.toggle_split(ctx);
        let msg = if app.workspace.is_split() {
            "Split view enabled"
        } else {
            "Split view disabled"
        };
        set_overlay_message(app, time, msg);
    }

    if app.bottom_bar_scale_editing || app.bottom_bar_index_editing {
        return;
    }

    if app.workspace.active_view().show_original_while_held != show_original_hold {
        app.workspace.active_view_mut().show_original_while_held = show_original_hold;
        app.workspace.active_view_mut().adjustments_dirty = true;
        if show_original_hold {
            set_overlay_message(app, time, "Shortcut: Show original");
        }
    }

    if toggle_toolbar {
        app.show_floating_toolbar = !app.show_floating_toolbar;
        set_overlay_message(app, time, "Shortcut: Toggle toolbar");
    }

    if app.show_filter_popup {
        if toggle_search {
            toggle_filter_popup(app);
            set_overlay_message(app, time, "Shortcut: Toggle filter popup");
        }
        if delete_current_file_permanently {
            open_delete_file_dialog(app, time);
        }
        if overwrite_with_adjustments {
            open_save_overwrite_dialog(app, time);
        }
        if reload_current_context {
            reload_current_context_like_overwrite(app, time);
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
        let text = format!("Shortcut: Sort type -> {}", sort_method_name(app.workspace.active_view().sort_method));
        set_overlay_message(app, time, &text);
    }
    if cycle_sort_next {
        cycle_sort_method(app, 1);
        let text = format!("Shortcut: Sort type -> {}", sort_method_name(app.workspace.active_view().sort_method));
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

    if delete_current_file_permanently {
        open_delete_file_dialog(app, time);
        return;
    }

    if reveal_in_explorer {
        reveal_current_in_explorer(app, time);
    }

    if overwrite_with_adjustments {
        open_save_overwrite_dialog(app, time);
        return;
    }

    if reload_current_context {
        reload_current_context_like_overwrite(app, time);
    }

    if rotate_clockwise {
        let active_view = app.workspace.active_view_mut();
        active_view.rotation_quarter_turns = (active_view.rotation_quarter_turns + 1) % 4;
        active_view.adjustments_dirty = true;
        set_overlay_message(app, time, "Shortcut: Rotate 90 deg CW");
    }

    if close_window {
        set_overlay_message(app, time, "Shortcut: Close window");
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }

    let allow_plain_adjustments = !alt_held && !show_original_hold;

    if allow_plain_adjustments {
        if let Some(multiplier) = saturation_down {
            let changed = {
                let saturation = &mut app.workspace.active_view_mut().adjustments.saturation;
                saturation.adjust_by(-crate::adjustments::saturation::SaturationAdjustment::STEP * multiplier)
            };
            show_adjustment_overlay(app, time, crate::adjustments::pipeline::AdjustmentTarget::Saturation);
            mark_adjustments_changed(app, changed);
        }
        if let Some(multiplier) = saturation_up {
            let changed = {
                let saturation = &mut app.workspace.active_view_mut().adjustments.saturation;
                saturation.adjust_by(crate::adjustments::saturation::SaturationAdjustment::STEP * multiplier)
            };
            show_adjustment_overlay(app, time, crate::adjustments::pipeline::AdjustmentTarget::Saturation);
            mark_adjustments_changed(app, changed);
        }

        if let Some(multiplier) = contrast_down {
            let changed = {
                let contrast = &mut app.workspace.active_view_mut().adjustments.contrast;
                contrast.adjust_by(-crate::adjustments::contrast::ContrastAdjustment::STEP * multiplier)
            };
            show_adjustment_overlay(app, time, crate::adjustments::pipeline::AdjustmentTarget::Contrast);
            mark_adjustments_changed(app, changed);
        }
        if let Some(multiplier) = contrast_up {
            let changed = {
                let contrast = &mut app.workspace.active_view_mut().adjustments.contrast;
                contrast.adjust_by(crate::adjustments::contrast::ContrastAdjustment::STEP * multiplier)
            };
            show_adjustment_overlay(app, time, crate::adjustments::pipeline::AdjustmentTarget::Contrast);
            mark_adjustments_changed(app, changed);
        }

        if let Some(multiplier) = gamma_down {
            let changed = {
                let gamma = &mut app.workspace.active_view_mut().adjustments.gamma;
                gamma.adjust_by(-crate::adjustments::gamma::GammaAdjustment::STEP * multiplier)
            };
            show_adjustment_overlay(app, time, crate::adjustments::pipeline::AdjustmentTarget::Gamma);
            mark_adjustments_changed(app, changed);
        }
        if let Some(multiplier) = gamma_up {
            let changed = {
                let gamma = &mut app.workspace.active_view_mut().adjustments.gamma;
                gamma.adjust_by(crate::adjustments::gamma::GammaAdjustment::STEP * multiplier)
            };
            show_adjustment_overlay(app, time, crate::adjustments::pipeline::AdjustmentTarget::Gamma);
            mark_adjustments_changed(app, changed);
        }

        if let Some(multiplier) = exposure_down {
            let changed = {
                let exposure = &mut app.workspace.active_view_mut().adjustments.exposure;
                exposure.adjust_by(-crate::adjustments::exposure::ExposureAdjustment::STEP * multiplier)
            };
            show_adjustment_overlay(app, time, crate::adjustments::pipeline::AdjustmentTarget::Exposure);
            mark_adjustments_changed(app, changed);
        }
        if let Some(multiplier) = exposure_up {
            let changed = {
                let exposure = &mut app.workspace.active_view_mut().adjustments.exposure;
                exposure.adjust_by(crate::adjustments::exposure::ExposureAdjustment::STEP * multiplier)
            };
            show_adjustment_overlay(app, time, crate::adjustments::pipeline::AdjustmentTarget::Exposure);
            mark_adjustments_changed(app, changed);
        }

        if let Some(multiplier) = highlights_down {
            let changed = {
                let highlights = &mut app.workspace.active_view_mut().adjustments.highlights;
                highlights.adjust_by(-crate::adjustments::highlights::HighlightsAdjustment::STEP * multiplier)
            };
            show_adjustment_overlay(app, time, crate::adjustments::pipeline::AdjustmentTarget::Highlights);
            mark_adjustments_changed(app, changed);
        }
        if let Some(multiplier) = highlights_up {
            let changed = {
                let highlights = &mut app.workspace.active_view_mut().adjustments.highlights;
                highlights.adjust_by(crate::adjustments::highlights::HighlightsAdjustment::STEP * multiplier)
            };
            show_adjustment_overlay(app, time, crate::adjustments::pipeline::AdjustmentTarget::Highlights);
            mark_adjustments_changed(app, changed);
        }

        if let Some(multiplier) = shadows_down {
            let changed = {
                let shadows = &mut app.workspace.active_view_mut().adjustments.shadows;
                shadows.adjust_by(-crate::adjustments::shadows::ShadowsAdjustment::STEP * multiplier)
            };
            show_adjustment_overlay(app, time, crate::adjustments::pipeline::AdjustmentTarget::Shadows);
            mark_adjustments_changed(app, changed);
        }
        if let Some(multiplier) = shadows_up {
            let changed = {
                let shadows = &mut app.workspace.active_view_mut().adjustments.shadows;
                shadows.adjust_by(crate::adjustments::shadows::ShadowsAdjustment::STEP * multiplier)
            };
            show_adjustment_overlay(app, time, crate::adjustments::pipeline::AdjustmentTarget::Shadows);
            mark_adjustments_changed(app, changed);
        }
    }

    if reset_all {
        let had_changes = app.workspace.active_view_mut().adjustments.has_adjustments();
        app.workspace.active_view_mut().adjustments.reset_all();
        set_overlay_message(app, time, "Shortcut: Reset adjustments");
        if had_changes {
            app.workspace.active_view_mut().adjustments_dirty = true;
        }
    }

}

fn mark_adjustments_changed(app: &mut ImageApp, changed: bool) {
    if changed {
        app.workspace.active_view_mut().adjustments_dirty = true;
    }
}

fn show_adjustment_overlay(
    app: &mut ImageApp,
    time: f64,
    target: crate::adjustments::pipeline::AdjustmentTarget,
) {
    let message = app.workspace.active_view_mut().adjustments.overlay_text_for(target);
    set_overlay_message(app, time, &message);
}

fn set_overlay_message(app: &mut ImageApp, time: f64, text: &str) {
    app.workspace.active_view_mut().overlay_last_changed = Some(time);
    app.workspace.active_view_mut().overlay_text = Some(text.to_string());
}

#[cfg(windows)]
fn reveal_in_explorer_windows(path: &std::path::Path) -> Result<(), ()> {
    use std::os::windows::ffi::OsStrExt;

    let wide_path: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    unsafe {
        let hr_init = CoInitializeEx(std::ptr::null_mut(), COINIT_APARTMENTTHREADED);
        let should_uninitialize = hr_init >= 0;
        let com_ready = hr_init >= 0 || hr_init == RPC_E_CHANGED_MODE;
        if !com_ready {
            return Err(());
        }

        let mut pidl: *mut core::ffi::c_void = std::ptr::null_mut();
        let hr_parse = SHParseDisplayName(
            wide_path.as_ptr(),
            std::ptr::null_mut(),
            &mut pidl,
            0,
            std::ptr::null_mut(),
        );

        if hr_parse < 0 || pidl.is_null() {
            if should_uninitialize {
                CoUninitialize();
            }
            return Err(());
        }

        // cidl=0 with a fully qualified item PIDL opens parent folder and selects that item.
        let hr_open = SHOpenFolderAndSelectItems(
            pidl,
            0,
            std::ptr::null(),
            0,
        );

        CoTaskMemFree(pidl);

        if should_uninitialize {
            CoUninitialize();
        }

        if hr_open >= 0 {
            Ok(())
        } else {
            Err(())
        }
    }
}

fn reveal_current_in_explorer(app: &mut ImageApp, time: f64) {
    let Some(path) = app.workspace.active_view_mut().current_file_path.as_ref() else {
        set_overlay_message(app, time, "Shortcut: No file to reveal");
        return;
    };

    #[cfg(windows)]
    {
        let absolute_path = if path.is_absolute() {
            path.clone()
        } else {
            std::env::current_dir()
                .map(|cwd| cwd.join(path))
                .unwrap_or_else(|_| path.clone())
        };

        let launched = reveal_in_explorer_windows(&absolute_path).is_ok();

        if launched {
            set_overlay_message(app, time, "Shortcut: Reveal in Explorer");
        } else {
            set_overlay_message(app, time, "Shortcut: Failed to open Explorer");
        }
    }

    #[cfg(not(windows))]
    {
        let _ = path;
        set_overlay_message(app, time, "Shortcut: Reveal is only available on Windows");
    }
}

fn rotate_rgba_90_cw(pixels: &[u8], width: usize, height: usize) -> Vec<u8> {
    let mut out = vec![0u8; pixels.len()];

    for y in 0..height {
        for x in 0..width {
            let src = (y * width + x) * 4;
            let dst_x = height - 1 - y;
            let dst_y = x;
            let dst = (dst_y * height + dst_x) * 4;
            out[dst..dst + 4].copy_from_slice(&pixels[src..src + 4]);
        }
    }

    out
}

fn apply_rotation_quarter_turns(
    mut pixels: Vec<u8>,
    mut width: u32,
    mut height: u32,
    quarter_turns: u8,
) -> (Vec<u8>, u32, u32) {
    let turns = quarter_turns % 4;
    for _ in 0..turns {
        let rotated = rotate_rgba_90_cw(&pixels, width as usize, height as usize);
        pixels = rotated;
        std::mem::swap(&mut width, &mut height);
    }

    (pixels, width, height)
}

/// Rebuilds GPU textures from original pixels with current adjustments applied.
/// Only runs when the `adjustments_dirty` flag is set, to avoid per-frame work.
pub fn rebuild_adjusted_textures(app: &mut ImageApp, _ctx: &egui::Context) {
    if !app.workspace.active_view_mut().adjustments_dirty {
        return;
    }
    app.workspace.active_view_mut().adjustments_dirty = false;

    let (width, height) = match app.workspace.active_view_mut().image_resolution {
        Some(res) => res,
        None => return,
    };

    // Extract needed data from app up-front to avoid borrow-checking issues
    let show_original = app.workspace.active_view().show_original_while_held;
    let has_adjustments = app.workspace.active_view().adjustments.has_adjustments();
    let adjustments = app.workspace.active_view().adjustments.clone();
    let rotation = app.workspace.active_view().rotation_quarter_turns;

    // Collect cloned frames to update them without holding the borrow
    let mut updated_images = Vec::new();
    for original in app.workspace.active_view().original_pixels.iter() {
        let base = if show_original {
            original.clone()
        } else if has_adjustments {
            adjustments.apply_all(original)
        } else {
            original.clone()
        };

        let (adjusted, draw_width, draw_height) = apply_rotation_quarter_turns(
            base,
            width,
            height,
            rotation,
        );

        let color_image = egui::ColorImage::from_rgba_unmultiplied(
            [draw_width as usize, draw_height as usize],
            &adjusted,
        );
        updated_images.push(color_image);
    }
    
    for (i, color_image) in updated_images.into_iter().enumerate() {
        if let Some(tex) = app.workspace.active_view_mut().frames.get_mut(i) {
            tex.set(color_image, egui::TextureOptions::LINEAR);
        }
    }
}

pub fn process_directory_scanning(app: &mut ImageApp) {
    for i in 0..app.workspace.views.len() {
        let mut latest_scan = None;
        while let Ok(result) = app.workspace.views[i].dir_res_rx.try_recv() {
            latest_scan = Some(result);
        }

        if let Some(scan) = latest_scan {
            let expected_id = app.workspace.views[i].scan_id.load(Ordering::Acquire);
            if scan.request_id != expected_id {
                continue;
            }
            app.workspace.views[i].current_folder = Some(scan.folder_path.clone());
            app.workspace.views[i].source_playlist = scan.playlist.clone();
            rebuild_active_playlist_and_reconcile_current(app, i);
        }
    }
}

pub fn handle_drag_and_drop(app: &mut ImageApp, ctx: &egui::Context) {
    if app.show_delete_file_dialog {
        return;
    }

    ctx.input(|i| {
        if let Some(dropped_file) = i.raw.dropped_files.first() {
            if let Some(path) = &dropped_file.path {
                let same_target = app
                    .workspace.active_view()
                    .current_file_path
                    .as_ref()
                    .is_some_and(|current| current == path);

                let same_directory = path
                    .parent()
                    .and_then(|parent| app.workspace.active_view().current_folder.as_ref().map(|folder| folder.as_path() == parent))
                    .unwrap_or(false);

                // Ignore self-drops of the currently loaded file to avoid unnecessary reloads
                // that reset zoom/pan state and trigger a full refresh pipeline.
                if same_target && same_directory {
                    return;
                }

                app.workspace.active_view_mut().preload.on_new_open();
                let mut should_scan = false;
                
                if let Some(parent) = path.parent() {
                    if Some(parent.to_path_buf()) == app.workspace.active_view_mut().current_folder {
                        if let Some(idx) = app.workspace.active_view_mut().active_playlist.iter().position(|p| p == path) {
                            app.workspace.active_view_mut().current_index = idx;
                        } else {
                            should_scan = true;
                        }
                    } else {
                        // --- THE FIX: PREVENT ASYNC RACE CONDITIONS ---
                        // Immediately invalidate the old folder's state
                        app.workspace.active_view_mut().current_folder = Some(parent.to_path_buf());
                        app.workspace.active_view_mut().source_playlist.clear();
                        app.workspace.active_view_mut().active_playlist.clear();
                        app.workspace.active_view_mut().current_index = 0;
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

fn has_overwritable_adjustment_changes(app: &ImageApp) -> bool {
    app.workspace.active_view().adjustments.has_adjustments() || app.workspace.active_view().rotation_quarter_turns != 0
}

fn infer_writable_image_format(path: &std::path::Path) -> Option<image::ImageFormat> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    match ext.as_str() {
        "png" => Some(image::ImageFormat::Png),
        "jpg" | "jpeg" => Some(image::ImageFormat::Jpeg),
        "webp" => Some(image::ImageFormat::WebP),
        "bmp" => Some(image::ImageFormat::Bmp),
        "tif" | "tiff" => Some(image::ImageFormat::Tiff),
        "tga" => Some(image::ImageFormat::Tga),
        _ => None,
    }
}

fn write_adjusted_current_frame(path: &std::path::Path, app: &mut ImageApp) -> Result<(), String> {
    let (base_width, base_height) = app
        .workspace.active_view()
        .image_resolution
        .ok_or_else(|| "No decoded image loaded".to_string())?;

    if app.workspace.active_view().original_pixels.len() != 1 {
        return Err("Overwrite currently supports static images only".to_string());
    }

    let format = infer_writable_image_format(path)
        .ok_or_else(|| "Unsupported target file format for overwrite".to_string())?;

    let mut pixels = app.workspace.active_view().original_pixels[0].clone();
    if app.workspace.active_view().adjustments.has_adjustments() {
        pixels = app.workspace.active_view().adjustments.apply_all(&pixels);
    }

    let (pixels, width, height) = apply_rotation_quarter_turns(
        pixels,
        base_width,
        base_height,
        app.workspace.active_view_mut().rotation_quarter_turns,
    );

    let expected_len = (width as usize)
        .saturating_mul(height as usize)
        .saturating_mul(4);
    if pixels.len() != expected_len {
        return Err("Adjusted pixel buffer size mismatch".to_string());
    }

    let rgba = image::RgbaImage::from_raw(width, height, pixels)
        .ok_or_else(|| "Failed to construct output image buffer".to_string())?;
    let out = image::DynamicImage::ImageRgba8(rgba);
    out.save_with_format(path, format)
        .map_err(|e| format!("Failed to overwrite file: {}", e))
}

fn overwrite_current_file_with_adjustments(app: &mut ImageApp, time: f64) {
    let Some(path) = app.workspace.active_view_mut().current_file_path.clone() else {
        set_overlay_message(app, time, "Shortcut: No file to save");
        return;
    };

    if !has_overwritable_adjustment_changes(app) {
        set_overlay_message(app, time, "Shortcut: No adjustments to save");
        return;
    }

    if app.workspace.active_view_mut().original_pixels.is_empty() || app.workspace.active_view_mut().image_resolution.is_none() {
        set_overlay_message(app, time, "Shortcut: Image is not ready to save");
        return;
    }

    match write_adjusted_current_frame(&path, app) {
        Ok(()) => {
            // Force fresh bytes from disk and re-sync playlist order/metadata after overwrite.
            reload_path_like_overwrite(app, path);
            set_overlay_message(app, time, "Shortcut: Saved current file");
        }
        Err(error) => {
            let text = format!("Shortcut: Save failed ({})", error);
            set_overlay_message(app, time, &text);
        }
    }
}

fn reload_path_like_overwrite(app: &mut ImageApp, path: std::path::PathBuf) {
    open_target(app, path);
}

fn reload_current_context_like_overwrite(app: &mut ImageApp, time: f64) {
    let Some(path) = app.workspace.active_view_mut().current_file_path.clone() else {
        set_overlay_message(app, time, "Shortcut: No file to reload");
        return;
    };

    reload_path_like_overwrite(app, path);
    set_overlay_message(app, time, "Shortcut: Reloaded current file and playlist");
}

