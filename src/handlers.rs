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
    app.workspace.active_view_mut().scanning_in_progress = true;

    let scan_root = app
        .workspace
        .active_view()
        .current_folder
        .clone()
        .or_else(|| target_path.parent().map(|parent| parent.to_path_buf()))
        .unwrap_or_else(|| target_path.clone());

    if app
        .workspace
        .active_view()
        .current_folder
        .as_ref()
        .is_some_and(|current| current.as_path() == scan_root.as_path())
    {
        persist_sort_preference_for_directory(app, &scan_root);
    } else {
        apply_sort_preference_for_directory(app, &scan_root);
    }

    let request_id = next_scan_request_id(app);
    let _ = app.workspace.active_view().dir_req_tx.send(crate::scanner::ScanRequest {
        target_path,
        scan_root,
        sort_method: app.workspace.active_view().sort_method,
        sort_order: app.workspace.active_view().sort_order,
        recursive: app.workspace.active_view().recursive_scan_enabled,
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

fn rescan_current_folder(app: &mut ImageApp) {
    if let Some(folder) = app.workspace.active_view().current_folder.clone() {
        let scan_target = folder.join("__folder_recursive_toggle__");
        request_directory_scan(app, scan_target);
        return;
    }

    if let Some(path) = current_sort_target_path(app) {
        request_directory_scan(app, path);
    }
}

pub fn toggle_recursive_scan(app: &mut ImageApp) {
    let next = !app.workspace.active_view().recursive_scan_enabled;
    app.workspace.active_view_mut().recursive_scan_enabled = next;
    rescan_current_folder(app);
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
    app.delete_file_dialog_targets.clear();
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

/// Open the permanent-delete confirmation dialog for the current grid selection.
/// Works for both the Default group and user-created groups.
pub fn open_delete_file_dialog_for_selection(app: &mut ImageApp, time: f64) {
    // Collect selected indices first (clone to release the borrow before we
    // reach the mutable parts of this function).
    let (selected_indices, playlist_snapshot): (Vec<usize>, Vec<std::path::PathBuf>) = {
        let grid = match app.workspace.playlist_grid.as_ref() {
            Some(g) => g,
            None => {
                set_overlay_message(app, time, "Shortcut: No files selected");
                return;
            }
        };

        if grid.selection.selected.is_empty() {
            set_overlay_message(app, time, "Shortcut: No files selected");
            return;
        }

        let indices: Vec<usize> = grid.selection.selected.iter().copied().collect();
        let playlist = app.workspace.active_view().active_playlist.clone();
        (indices, playlist)
    };

    let selected_paths: Vec<std::path::PathBuf> = selected_indices
        .iter()
        .filter_map(|idx| playlist_snapshot.get(*idx).cloned())
        .collect();

    if selected_paths.is_empty() {
        set_overlay_message(app, time, "Shortcut: No files selected");
        return;
    }

    close_filter_popup(app);
    app.show_sort_menu = false;
    app.sort_menu_pos = None;

    app.show_delete_file_dialog = true;
    app.delete_file_dialog_targets = selected_paths.clone();
    // Also set the single-target field so the dialog falls back correctly for
    // the single-file case (len == 1).
    app.delete_file_dialog_target = selected_paths.into_iter().next();
    app.delete_file_dialog_selection = ConfirmationSelection::Confirm;
}

/// Confirm permanent deletion for the playlist/group view.
/// Deletes all files listed in `delete_file_dialog_targets` from disk and
/// removes them from every group playlist, then refreshes the active view.
pub fn confirm_delete_file_dialog_playlist(app: &mut ImageApp, time: f64) {
    let targets = app.delete_file_dialog_targets.clone();
    cancel_delete_file_dialog(app);

    if targets.is_empty() {
        return;
    }

    // 1. Delete all target files from disk.
    let mut deleted: Vec<std::path::PathBuf> = Vec::new();
    let mut failed: Vec<String> = Vec::new();

    for path in &targets {
        match std::fs::remove_file(path) {
            Ok(()) => deleted.push(path.clone()),
            Err(e) => failed.push(format!("{}: {}", path.display(), e)),
        }
    }

    if deleted.is_empty() {
        let text = format!("Delete failed: {}", failed.join(", "));
        set_overlay_message(app, time, &text);
        return;
    }

    let active_group_id = app.workspace.group_tabs.selected_id;

    // 2. Before patching stored group states, snapshot the current active view
    //    state (including the current user-group playlist) and save it into the
    //    group_tabs store so the loop below operates on fully up-to-date data.
    {
        let current_view_state =
            crate::groups::GroupPlaylistState::from_view(app.workspace.active_view());
        app.workspace
            .group_tabs
            .set_group_playlist(active_group_id, current_view_state);
    }

    // 3. Patch every group's stored playlist — remove deleted paths from source
    //    and rebuild the active playlist for each affected group.
    let all_group_ids: Vec<u32> = {
        let mut ids = vec![crate::groups::DEFAULT_GROUP_ID];
        ids.extend(app.workspace.group_tabs.user_groups.iter().map(|g| g.id));
        ids
    };

    for group_id in &all_group_ids {
        if let Some(state) = app.workspace.group_tabs.group_playlist_mut(*group_id) {
            let before = state.source_playlist.len();
            state.source_playlist.retain(|p| !deleted.iter().any(|d| d == p));
            if state.source_playlist.len() != before {
                state.rebuild_active_playlist();
            }
        }
    }

    // 4. Re-apply the patched active group state to the live view so the grid
    //    reflects the deletions immediately.
    if let Some(patched_state) = app
        .workspace
        .group_tabs
        .group_playlist(active_group_id)
        .cloned()
    {
        apply_group_playlist_state(app, &patched_state);
    }

    // 5. Evict deleted paths from the thumbnail cache so they don't linger
    //    as stale entries when switching groups.
    {
        let playlist_snapshot = app.workspace.active_view().active_playlist.clone();
        if let Some(grid) = app.workspace.playlist_grid.as_mut() {
            for path in &deleted {
                grid.thumbnail_cache.remove(path);
                grid.pending_requests.remove(path);
            }
            // Selection was already cleared by apply_group_playlist_state above;
            // refresh the size totals with the updated playlist.
            grid.refresh_total_size_cache(&playlist_snapshot);
        }
    }

    // 6. For the default group, trigger a directory rescan so that the source
    //    playlist is authoritative from disk again (the scan handler also keeps
    //    the DEFAULT group store in sync). For user groups, the scan result is
    //    intentionally not applied to the active view by process_directory_scanning,
    //    so it would only serve to update the DEFAULT group store — which we
    //    already patched in step 3.
    let is_default_group = active_group_id == crate::groups::DEFAULT_GROUP_ID;
    if is_default_group {
        if let Some(scan_target) = app
            .workspace
            .active_view()
            .current_folder
            .clone()
            .map(|folder| folder.join("__delete_refresh__"))
        {
            request_directory_scan(app, scan_target);
        }
    }

    // 7. Overlay message.
    let text = if failed.is_empty() {
        if deleted.len() == 1 {
            let name = deleted[0]
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| deleted[0].to_string_lossy().into_owned());
            format!("Deleted {}", name)
        } else {
            format!("Deleted {} files", deleted.len())
        }
    } else {
        format!("Deleted {}, {} failed", deleted.len(), failed.len())
    };
    set_overlay_message(app, time, &text);
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
        let is_playlist_grid =
            app.workspace.content_mode == crate::workspace::ContentMode::PlaylistGrid;
        match app.delete_file_dialog_selection {
            ConfirmationSelection::Cancel => cancel_delete_file_dialog(app),
            ConfirmationSelection::Confirm => {
                if is_playlist_grid {
                    confirm_delete_file_dialog_playlist(app, time);
                } else {
                    confirm_delete_file_dialog(app, time);
                }
            }
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

fn is_group_restricted(app: &ImageApp) -> bool {
    app.workspace.content_mode == crate::workspace::ContentMode::PlaylistGrid
        && app.workspace.group_tabs.selected_id != crate::groups::DEFAULT_GROUP_ID
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

fn apply_group_playlist_state(app: &mut ImageApp, state: &crate::groups::GroupPlaylistState) {
    state.apply_to_view(app.workspace.active_view_mut());

    let playlist_snapshot = app.workspace.active_view().active_playlist.clone();
    let current_index = app.workspace.active_view().current_index;
    app.workspace.active_view_mut().preload.on_playlist_updated(
        &playlist_snapshot,
        current_index,
        app.settings.loop_playlist,
        None,
    );

    if let Some(grid) = app.workspace.playlist_grid.as_mut() {
        grid.selection.clear();
        grid.refresh_total_size_cache(&playlist_snapshot);
        if playlist_snapshot.is_empty() {
            grid.scroll_to_index = None;
        } else {
            grid.scroll_to_index = Some(current_index);
        }
    }
}

pub fn switch_group(app: &mut ImageApp, group_id: u32) {
    let current_group_id = app.workspace.group_tabs.selected_id;
    if current_group_id == group_id {
        return;
    }

    let current_state = crate::groups::GroupPlaylistState::from_view(app.workspace.active_view());
    app.workspace
        .group_tabs
        .set_group_playlist(current_group_id, current_state);

    app.workspace.group_tabs.ensure_group_playlist(group_id);
    app.workspace.group_tabs.select_group(group_id);

    let next_state = app
        .workspace
        .group_tabs
        .group_playlist(group_id)
        .cloned()
        .unwrap_or_else(crate::groups::GroupPlaylistState::new);

    apply_group_playlist_state(app, &next_state);
}

pub fn close_group_tab(app: &mut ImageApp, group_id: u32) {
    let was_active = app.workspace.group_tabs.selected_id == group_id;

    app.workspace.group_tabs.close_group(group_id);

    if !was_active {
        return;
    }

    let next_id = app.workspace.group_tabs.selected_id;
    app.workspace.group_tabs.ensure_group_playlist(next_id);

    let next_state = app
        .workspace
        .group_tabs
        .group_playlist(next_id)
        .cloned()
        .unwrap_or_else(crate::groups::GroupPlaylistState::new);

    apply_group_playlist_state(app, &next_state);
}

fn resolve_group_assign_target(app: &mut ImageApp) -> crate::groups::GroupAssignTarget {
    match app.group_assign_target {
        crate::groups::GroupAssignTarget::Group(id) if app.workspace.group_tabs.has_group(id) => {
            app.group_assign_target
        }
        _ => {
            app.group_assign_target = crate::groups::GroupAssignTarget::AskEveryTime;
            app.group_assign_target
        }
    }
}

fn open_group_assign_prompt(app: &mut ImageApp, path: std::path::PathBuf) {
    app.show_group_assign_prompt = true;
    app.group_assign_prompt_path = Some(path);
    app.group_assign_prompt_source_group = app.workspace.group_tabs.selected_id;
}

fn close_group_assign_prompt(app: &mut ImageApp) {
    app.show_group_assign_prompt = false;
    app.group_assign_prompt_path = None;
}

pub fn apply_group_assign_prompt(app: &mut ImageApp, target_group_id: u32, time: f64) {
    let Some(path) = app.group_assign_prompt_path.clone() else {
        close_group_assign_prompt(app);
        return;
    };

    let source_group_id = app.group_assign_prompt_source_group;
    let needs_animation = source_group_id != crate::groups::DEFAULT_GROUP_ID
        && app.workspace.content_mode == crate::workspace::ContentMode::Canvas;
    let target_name = app
        .workspace
        .group_tabs
        .group_name(target_group_id)
        .unwrap_or_else(|| "group".to_string());

    let paths = vec![path];
    let success = transfer_items_between_groups(app, source_group_id, target_group_id, &paths, time);
    close_group_assign_prompt(app);

    if success {
        set_overlay_message(app, time, &format!("Moved to {}", target_name));
        if needs_animation {
            start_move_animation(app, time);
        }
    }
}

pub fn handle_add_to_group_shortcut(app: &mut ImageApp, time: f64) {
    if !app.settings.groups_enabled {
        return;
    }

    let Some(path) = app.workspace.active_view().current_file_path.clone() else {
        return;
    };

    match resolve_group_assign_target(app) {
        crate::groups::GroupAssignTarget::AskEveryTime => {
            open_group_assign_prompt(app, path);
        }
        crate::groups::GroupAssignTarget::Group(target_group_id) => {
            let source_group_id = app.workspace.group_tabs.selected_id;
            let needs_animation = source_group_id != crate::groups::DEFAULT_GROUP_ID
                && app.workspace.content_mode == crate::workspace::ContentMode::Canvas;
            let target_name = app
                .workspace
                .group_tabs
                .group_name(target_group_id)
                .unwrap_or_else(|| "group".to_string());

            let paths = vec![path];
            let success = transfer_items_between_groups(app, source_group_id, target_group_id, &paths, time);

            if success {
                set_overlay_message(app, time, &format!("Moved to {}", target_name));
                if needs_animation {
                    start_move_animation(app, time);
                }
            }
        }
    }
}

pub fn handle_move_to_default_shortcut(app: &mut ImageApp, time: f64) {
    if !app.settings.groups_enabled {
        return;
    }

    let source_group_id = app.workspace.group_tabs.selected_id;
    if source_group_id == crate::groups::DEFAULT_GROUP_ID {
        set_overlay_message(app, time, "Already in Default");
        return;
    }

    let Some(path) = app.workspace.active_view().current_file_path.clone() else {
        return;
    };

    let is_canvas = app.workspace.content_mode == crate::workspace::ContentMode::Canvas;
    let paths = vec![path];
    let success = transfer_items_between_groups(
        app,
        source_group_id,
        crate::groups::DEFAULT_GROUP_ID,
        &paths,
        time,
    );

    if success {
        set_overlay_message(app, time, "Moved to Default");
        if is_canvas {
            start_move_animation(app, time);
        }
    }
}

// --- Move animation constants (each phase is individually tunable) ---
const MOVE_ANIM_SCALE_DOWN_DURATION: f64 = 0.09;
const MOVE_ANIM_PAUSE_DURATION: f64 = 0.04;
const MOVE_ANIM_SLIDE_UP_DURATION: f64 = 0.10;

pub const MOVE_ANIM_TOTAL_DURATION: f64 =
    MOVE_ANIM_SCALE_DOWN_DURATION + MOVE_ANIM_PAUSE_DURATION + MOVE_ANIM_SLIDE_UP_DURATION;
pub const MOVE_ANIM_SCALE_FACTOR: f32 = 0.92;
pub const MOVE_ANIM_SLIDE_DISTANCE: f32 = 80.0;

/// Returns (scale_factor, y_offset, alpha) for the current animation frame.
pub fn move_anim_values(elapsed: f64) -> (f32, f32, u8) {
    if elapsed < MOVE_ANIM_SCALE_DOWN_DURATION {
        // Phase 1: scale down
        let t = (elapsed / MOVE_ANIM_SCALE_DOWN_DURATION) as f32;
        let scale = 1.0 + (MOVE_ANIM_SCALE_FACTOR - 1.0) * t;
        (scale, 0.0, 255)
    } else if elapsed < MOVE_ANIM_SCALE_DOWN_DURATION + MOVE_ANIM_PAUSE_DURATION {
        // Phase 2: pause
        (MOVE_ANIM_SCALE_FACTOR, 0.0, 255)
    } else {
        // Phase 3: slide up + fade out
        let phase_elapsed = elapsed - MOVE_ANIM_SCALE_DOWN_DURATION - MOVE_ANIM_PAUSE_DURATION;
        let t = (phase_elapsed / MOVE_ANIM_SLIDE_UP_DURATION).min(1.0) as f32;
        let y_offset = -MOVE_ANIM_SLIDE_DISTANCE * t;
        let alpha = ((1.0 - t) * 255.0) as u8;
        (MOVE_ANIM_SCALE_FACTOR, y_offset, alpha)
    }
}

fn start_move_animation(app: &mut ImageApp, time: f64) {
    let view = app.workspace.active_view();
    let playlist_len = view.active_playlist.len();
    let direction = if playlist_len == 0 {
        0
    } else {
        // current_index was already clamped by apply_group_playlist_state.
        // If we're at the end, go backward; otherwise go forward.
        // direction == 0 means clear view (playlist empty).
        1
    };

    app.workspace.active_view_mut().move_anim_start = Some(time);
    app.workspace.active_view_mut().move_anim_direction = direction;
}

pub fn process_move_animation(app: &mut ImageApp, ctx: &egui::Context) {
    let view = app.workspace.active_view();
    let Some(start) = view.move_anim_start else {
        return;
    };

    let now = ctx.input(|i| i.time);
    let elapsed = now - start;
    if elapsed < MOVE_ANIM_TOTAL_DURATION {
        ctx.request_repaint();
        return;
    }

    // Animation complete — advance to the next image.
    let direction = app.workspace.active_view().move_anim_direction;
    app.workspace.active_view_mut().move_anim_start = None;
    app.workspace.active_view_mut().move_anim_direction = 0;

    if direction == 0 || app.workspace.active_view().active_playlist.is_empty() {
        let index = app.workspace.active_view_index;
        clear_current_view_for_empty_playlist(app, index);
        return;
    }

    let target_path = app.workspace.active_view().active_playlist
        [app.workspace.active_view().current_index]
        .clone();
    load_target_file(app, target_path);
}

fn transfer_items_between_groups(
    app: &mut ImageApp,
    source_group_id: u32,
    target_group_id: u32,
    paths: &[std::path::PathBuf],
    time: f64,
) -> bool {
    if source_group_id == target_group_id || paths.is_empty() {
        return false;
    }

    let active_group_id = app.workspace.group_tabs.selected_id;
    let mut updated: std::collections::HashMap<u32, crate::groups::GroupPlaylistState> =
        std::collections::HashMap::new();

    fn load_group_state(
        app: &ImageApp,
        updated: &std::collections::HashMap<u32, crate::groups::GroupPlaylistState>,
        active_group_id: u32,
        group_id: u32,
    ) -> crate::groups::GroupPlaylistState {
        if let Some(state) = updated.get(&group_id) {
            return state.clone();
        }
        if group_id == active_group_id {
            return crate::groups::GroupPlaylistState::from_view(app.workspace.active_view());
        }
        app.workspace
            .group_tabs
            .group_playlist(group_id)
            .cloned()
            .unwrap_or_else(crate::groups::GroupPlaylistState::new)
    }

    let user_group_ids: Vec<u32> = app
        .workspace
        .group_tabs
        .user_groups
        .iter()
        .map(|group| group.id)
        .collect();

    let source_is_default = source_group_id == crate::groups::DEFAULT_GROUP_ID;
    let target_is_default = target_group_id == crate::groups::DEFAULT_GROUP_ID;

    if source_is_default && !target_is_default {
        for group_id in user_group_ids.iter().copied().filter(|id| *id != target_group_id) {
            let state = load_group_state(app, &updated, active_group_id, group_id);
            let duplicate_count = paths
                .iter()
                .filter(|path| state.source_playlist.iter().any(|existing| existing == *path))
                .count();

            if duplicate_count > 0 {
                let label = if duplicate_count == 1 { "Item" } else { "Items" };
                let message = format!("{} already exist in Group {}", label, group_id);
                app.notifications.show(time, message);
                return false;
            }
        }
    }

    if !target_is_default {
        app.workspace.group_tabs.ensure_group_playlist(target_group_id);
        let mut target_state = load_group_state(app, &updated, active_group_id, target_group_id);

        let added = target_state.add_items(paths);
        if added > 0 {
            target_state.rebuild_active_playlist();
        }

        updated.insert(target_group_id, target_state);
    }

    let groups_to_clear: Vec<u32> = if source_is_default {
        Vec::new()
    } else if target_is_default {
        user_group_ids
    } else {
        user_group_ids
            .into_iter()
            .filter(|id| *id != target_group_id)
            .collect()
    };

    for group_id in groups_to_clear {
        let mut state = load_group_state(app, &updated, active_group_id, group_id);
        if state.remove_items(paths) > 0 {
            state.rebuild_active_playlist();
            updated.insert(group_id, state);
        }
    }

    let mut active_state: Option<crate::groups::GroupPlaylistState> = None;
    for (group_id, state) in updated.into_iter() {
        if group_id == active_group_id {
            active_state = Some(state.clone());
        }
        app.workspace.group_tabs.set_group_playlist(group_id, state);
    }

    if let Some(state) = active_state {
        apply_group_playlist_state(app, &state);
    }

    true
}

pub fn handle_group_drop(app: &mut ImageApp, target_group_id: u32, time: f64) {
    let Some(payload) = app.group_drag_payload.take() else {
        return;
    };

    transfer_items_between_groups(
        app,
        payload.source_group_id,
        target_group_id,
        &payload.paths,
        time,
    );

}

/// Queues both image loading and directory scanning through the same runtime paths.
pub fn open_target(app: &mut ImageApp, path: std::path::PathBuf) {
    // Opening a new target starts a fresh context; no previous preload on first entry.
    app.workspace.group_tabs.reset_for_new_folder();
    app.workspace.content_mode = crate::workspace::ContentMode::Canvas;
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
    active_view.original_pixels.clear();
    active_view.rotation_quarter_turns = 0;
    active_view.overlay_last_changed = None;
    active_view.overlay_text = None;
    active_view.show_original_while_held = false;

    if active_view.carry_adjustments && active_view.adjustments.has_adjustments() {
        // Keep the current pipeline; mark dirty so it is re-applied once the
        // new image's pixels are available.
        active_view.adjustments_dirty = true;
    } else {
        active_view.adjustments.reset_all();
        active_view.adjustments_dirty = false;
    }
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

/// Fully resets the active view to a fresh-open state, as if the app was just launched.
/// In split view this only affects the active pane.
pub fn clear_active_view(app: &mut ImageApp) {
    // Close any open popups/menus that reference the current file.
    close_filter_popup(app);
    app.show_sort_menu = false;
    app.sort_menu_pos = None;

    let index = app.workspace.active_view_index;
    let view = &mut app.workspace.views[index];

    // Invalidate in-flight background loads so stale results are discarded.
    let _ = view.load_id.fetch_add(1, std::sync::atomic::Ordering::AcqRel);

    // Image state
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

    // Camera state
    view.auto_fit = true;
    view.scale = 1.0;
    view.pan = egui::Vec2::ZERO;
    view.target_scale = None;
    view.target_pan = None;
    view.reset_start_time = None;

    // Playlist / folder state
    view.current_folder = None;
    view.source_playlist.clear();
    view.active_playlist.clear();
    view.current_index = 0;
    view.filter = crate::state::FilterState::default();

    // Sort state — reset to defaults
    view.sort_method = crate::scanner::SortMethod::Natural;
    view.sort_order = crate::scanner::default_order_for(crate::scanner::SortMethod::Natural);
    view.recursive_scan_enabled = false;
    view.scanning_in_progress = false;

    // Adjustment state
    view.original_pixels.clear();
    view.adjustments.reset_all();
    view.adjustments_dirty = false;
    view.rotation_quarter_turns = 0;
    view.carry_adjustments = false;
    view.overlay_last_changed = None;
    view.overlay_text = None;
    view.show_original_while_held = false;

    // Preload ring — discard all cached data
    view.preload.on_new_open();

    app.workspace.content_mode = crate::workspace::ContentMode::Empty;
    if let Some(grid) = app.workspace.playlist_grid.as_mut() {
        grid.clear_for_new_folder();
    }

    // Clear the title cache so the window title updates immediately.
    app.cached_title.clear();
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
        if let Some(grid) = app.workspace.playlist_grid.as_mut() {
            grid.selection.clear();
            grid.refresh_total_size_cache(&playlist_snapshot);
        }
        return;
    }

    let target_index = previous_path
        .as_ref()
        .and_then(|path| app.workspace.views[index].active_playlist.iter().position(|p| p == path))
        .unwrap_or(0);
    let target_path = app.workspace.views[index].active_playlist[target_index].clone();
    let changed_target = previous_path.as_ref() != Some(&target_path);

    app.workspace.views[index].current_index = target_index;
    if changed_target && app.workspace.content_mode == crate::workspace::ContentMode::Canvas {
        // Force the file to load on active view?
        // Wait, `load_target_file` loads on active view only!
        // For simplicity let's save the current view, switch to index, load, switch back.
        let active = app.workspace.active_view_index;
        app.workspace.active_view_index = index;
        load_target_file(app, target_path);
        app.workspace.active_view_index = active;
    }

    let playlist_snapshot = app.workspace.views[index].active_playlist.clone();
    let current_path = app.workspace.views[index].current_file_path.clone();
    let current_index = app.workspace.views[index].current_index;
    let loop_playlist = app.settings.loop_playlist;

    app.workspace.views[index].preload.on_playlist_updated(
        &playlist_snapshot,
        current_index,
        loop_playlist,
        current_path.as_ref(),
    );

    if let Some(grid) = app.workspace.playlist_grid.as_mut() {
        grid.refresh_total_size_cache(&playlist_snapshot);
    }
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

    // Block most input while a move animation is playing.
    if app.workspace.active_view().move_anim_start.is_some() {
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
            shortcuts.clear_active_view.is_pressed(i),
            shortcuts.return_to_playlist.is_pressed(i),
            shortcuts.add_to_group.is_pressed(i),
            shortcuts.move_to_default.is_pressed(i),
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
        clear_view,
        return_to_playlist_pressed,
        add_to_group,
        move_to_default,
    ) = input;

    let is_playlist_grid = app.workspace.content_mode == crate::workspace::ContentMode::PlaylistGrid;

    if is_playlist_grid {
        if clear_view {
            clear_active_view(app);
            return;
        }

        if reload_current_context {
            refresh_current_context(app, time);
        }

        if app.show_filter_popup {
            if toggle_search {
                toggle_filter_popup(app);
                set_overlay_message(app, time, "Shortcut: Toggle filter popup");
            }
            return;
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
            open_delete_file_dialog_for_selection(app, time);
            return;
        }

        if close_window {
            set_overlay_message(app, time, "Shortcut: Close window");
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }

        return;
    }

    let is_split_toggle = ctx.input(|i| i.modifiers.alt && i.key_pressed(egui::Key::C));

    if is_split_toggle {
        let was_split = app.workspace.is_split();
        app.workspace.toggle_split(ctx);
        if app.workspace.is_split() && !was_split {
            app.split_pan_zoom_sync_enabled = crate::sync::pan_zoom::can_enable_sync(app);
            app.split_pan_zoom_sync_user_disabled = false;
        } else if !app.workspace.is_split() {
            app.split_pan_zoom_sync_enabled = false;
            app.split_pan_zoom_sync_user_disabled = false;
        }
        let msg = if app.workspace.is_split() {
            "Split view enabled"
        } else {
            "Split view disabled"
        };
        set_overlay_message(app, time, msg);
    }

    if clear_view {
        clear_active_view(app);
        return;
    }

    // Esc (no modifiers): return to playlist grid if we came from one.
    if return_to_playlist_pressed {
        if app.workspace.content_mode == crate::workspace::ContentMode::Canvas
            && app.workspace.playlist_grid.is_some()
            && app.workspace.active_view().current_folder.is_some()
        {
            return_to_playlist_view(app);
            return;
        }
    }

    if move_to_default {
        handle_move_to_default_shortcut(app, time);
        return;
    }

    if add_to_group {
        handle_add_to_group_shortcut(app, time);
        return;
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
            refresh_current_context(app, time);
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
        refresh_current_context(app, time);
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

    let (width, height) = match app.workspace.active_view_mut().image_resolution {
        Some(res) => res,
        // Image not loaded yet; keep the dirty flag so we retry next frame.
        None => return,
    };
    app.workspace.active_view_mut().adjustments_dirty = false;

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
            app.workspace.views[i].scanning_in_progress = false;
            app.workspace.views[i].current_folder = Some(scan.folder_path.clone());

            let is_active_view = i == app.workspace.active_view_index;
            let default_selected = app.workspace.group_tabs.selected_id == crate::groups::DEFAULT_GROUP_ID;

            if is_active_view && !default_selected {
                app.workspace
                    .group_tabs
                    .ensure_group_playlist(crate::groups::DEFAULT_GROUP_ID);
                if let Some(state) = app
                    .workspace
                    .group_tabs
                    .group_playlist_mut(crate::groups::DEFAULT_GROUP_ID)
                {
                    state.source_playlist = scan.playlist.clone();
                    state.rebuild_active_playlist();
                }
                continue;
            }

            app.workspace.views[i].source_playlist = scan.playlist.clone();
            rebuild_active_playlist_and_reconcile_current(app, i);

            if is_active_view && default_selected {
                let updated = crate::groups::GroupPlaylistState::from_view(&app.workspace.views[i]);
                app.workspace
                    .group_tabs
                    .set_group_playlist(crate::groups::DEFAULT_GROUP_ID, updated);
            }
        }
    }
}

pub fn handle_drag_and_drop(app: &mut ImageApp, ctx: &egui::Context) {
    if is_group_restricted(app) {
        return;
    }
    if app.show_delete_file_dialog {
        return;
    }

    // We need to call open_folder outside the ctx.input closure (it borrows ctx
    // mutably), so collect the dropped path first.
    let dropped_path = ctx.input(|i| {
        i.raw.dropped_files.first().and_then(|f| f.path.clone())
    });

    let Some(path) = dropped_path else {
        return;
    };

    // --- Dropped a folder: open it in playlist-grid mode. ---
    if path.is_dir() {
        open_folder(app, ctx, path);
        return;
    }

    // --- Dropped a file: open in canvas mode and set up the grid so Esc works. ---

    // Ignore self-drops of the currently loaded file.
    let same_target = app
        .workspace
        .active_view()
        .current_file_path
        .as_ref()
        .is_some_and(|current| current == &path);

    let parent_folder = path.parent().map(|p| p.to_path_buf());

    let same_directory = parent_folder.as_ref().and_then(|parent| {
        app.workspace
            .active_view()
            .current_folder
            .as_ref()
            .map(|folder| folder.as_path() == parent.as_path())
    }).unwrap_or(false);

    if same_target && same_directory {
        return;
    }

    let new_folder = parent_folder.as_ref().map_or(true, |parent| {
        app.workspace
            .active_view()
            .current_folder
            .as_ref()
            .map_or(true, |current| current.as_path() != parent.as_path())
    });

    // Switch to canvas mode.
    app.workspace.content_mode = crate::workspace::ContentMode::Canvas;

    // Always ensure the playlist grid exists so Esc → folder view works.
    if app.workspace.playlist_grid.is_none() {
        app.workspace.playlist_grid = Some(crate::playlist_grid::PlaylistGridState::new(ctx));
    }

    if new_folder {
        // New parent directory: reset groups and grid, update folder.
        app.workspace.group_tabs.reset_for_new_folder();
        if let Some(grid) = app.workspace.playlist_grid.as_mut() {
            grid.clear_for_new_folder();
        }

        if let Some(ref parent) = parent_folder {
            app.workspace.active_view_mut().current_folder = Some(parent.clone());
        }
        app.workspace.active_view_mut().source_playlist.clear();
        app.workspace.active_view_mut().active_playlist.clear();
        app.workspace.active_view_mut().current_index = 0;
    }

    app.workspace.active_view_mut().preload.on_new_open();
    load_target_file(app, path.clone());
    request_directory_scan(app, path);

    app.cached_title.clear();
}

pub fn handle_browse_file_request(app: &mut ImageApp) {
    if is_group_restricted(app) {
        return;
    }
    // Check all views, not just the active one, in case a split pane triggered it.
    let mut requested_view_index = None;
    for (i, view) in app.workspace.views.iter_mut().enumerate() {
        if view.browse_file_requested {
            view.browse_file_requested = false;
            requested_view_index = Some(i);
            break;
        }
    }

    let Some(view_index) = requested_view_index else {
        return;
    };

    let dialog = rfd::FileDialog::new()
        .set_title("Open Image")
        .add_filter(
            "Images",
            &[
                "webp", "avif", "heic", "heif", "hif", "jxl", "png", "jpg",
                "jpeg", "gif", "tif", "tiff", "bmp", "ico",
            ],
        )
        .add_filter("All Files", &["*"]);

    if let Some(path) = dialog.pick_file() {
        // Ensure the correct view is active before loading.
        let prev_active = app.workspace.active_view_index;
        app.workspace.active_view_index = view_index;
        open_target(app, path);
        // Restore the original active view if it was different and we're in split mode.
        if !app.workspace.is_split() {
            app.workspace.active_view_index = prev_active;
        }
    }
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

pub fn refresh_current_context(app: &mut ImageApp, time: f64) {
    if app.workspace.content_mode == crate::workspace::ContentMode::PlaylistGrid {
        let Some(folder) = app.workspace.active_view().current_folder.clone() else {
            set_overlay_message(app, time, "Shortcut: No folder to refresh");
            return;
        };

        let scan_target = folder.join("__folder_refresh_target__");
        request_directory_scan(app, scan_target);
        set_overlay_message(app, time, "Shortcut: Refreshed folder");
        return;
    }

    let Some(path) = app.workspace.active_view_mut().current_file_path.clone() else {
        set_overlay_message(app, time, "Shortcut: No file to refresh");
        return;
    };

    reload_path_like_overwrite(app, path);
    set_overlay_message(app, time, "Shortcut: Refreshed current file and playlist");
}

// ── Playlist Grid Handlers ───────────────────────────────────────────────

/// Open a folder and switch to the playlist grid view.
pub fn open_folder(app: &mut ImageApp, ctx: &egui::Context, folder_path: std::path::PathBuf) {
    if app.workspace.playlist_grid.is_none() {
        app.workspace.playlist_grid = Some(crate::playlist_grid::PlaylistGridState::new(ctx));
    }
    if let Some(grid) = app.workspace.playlist_grid.as_mut() {
        grid.clear_for_new_folder();
    }
    app.workspace.group_tabs.reset_for_new_folder();
    app.workspace.content_mode = crate::workspace::ContentMode::PlaylistGrid;
    let scan_target = folder_path.join("__folder_open_target__");
    app.workspace.active_view_mut().current_folder = Some(folder_path);
    app.workspace.active_view_mut().source_playlist.clear();
    app.workspace.active_view_mut().active_playlist.clear();
    app.workspace.active_view_mut().current_index = 0;
    request_directory_scan(app, scan_target);
    let index = app.workspace.active_view_index;
    clear_current_view_for_empty_playlist(app, index);
    if app.workspace.is_split() {
        app.workspace.views.truncate(1);
        app.workspace.active_view_index = 0;
    }
    app.cached_title.clear();
}

/// Handle the "Open Folder" button / browse-folder request.
pub fn handle_browse_folder_request(app: &mut ImageApp, ctx: &egui::Context) {
    if is_group_restricted(app) {
        return;
    }
    let mut requested = false;
    for view in app.workspace.views.iter_mut() {
        if view.browse_folder_requested {
            view.browse_folder_requested = false;
            requested = true;
            break;
        }
    }
    if !requested { return; }
    let dialog = rfd::FileDialog::new().set_title("Open Folder");
    if let Some(folder) = dialog.pick_folder() {
        open_folder(app, ctx, folder);
    }
}

/// Open a specific image from the playlist grid (double-click).
pub fn playlist_grid_open_image(app: &mut ImageApp, path: std::path::PathBuf, index: usize) {
    app.workspace.content_mode = crate::workspace::ContentMode::Canvas;
    app.workspace.active_view_mut().current_index = index;
    app.workspace.active_view_mut().preload.on_new_open();
    load_target_file(app, path.clone());
    request_directory_scan(app, path);
}

/// Return from canvas mode to the playlist grid view.
/// The previously viewed image stays selected and the grid scrolls to it.
pub fn return_to_playlist_view(app: &mut ImageApp) {
    let current_index = app.workspace.active_view().current_index;
    let _ = app.workspace.active_view_mut().load_id.fetch_add(1, Ordering::AcqRel);
    let view = app.workspace.active_view_mut();
    view.frames.clear();
    view.frame_durations.clear();
    view.current_frame = 0;
    view.last_frame_time = None;
    view.image_resolution = None;
    view.image_density = None;
    view.load_error = None;
    view.current_file_path = None;
    view.current_file_name.clear();
    view.current_file_size_bytes = None;
    view.original_pixels.clear();
    view.adjustments.reset_all();
    view.adjustments_dirty = false;
    view.rotation_quarter_turns = 0;
    view.overlay_last_changed = None;
    view.overlay_text = None;
    view.show_original_while_held = false;
    view.auto_fit = true;
    view.scale = 1.0;
    view.pan = egui::Vec2::ZERO;
    view.target_scale = None;
    view.target_pan = None;
    view.reset_start_time = None;
    view.preload.on_new_open();
    app.workspace.content_mode = crate::workspace::ContentMode::PlaylistGrid;
    let playlist_snapshot = app.workspace.active_view().active_playlist.clone();
    if let Some(grid) = app.workspace.playlist_grid.as_mut() {
        grid.selection.select_single(current_index);
        grid.scroll_to_index = Some(current_index);
        grid.refresh_selected_size_cache(&playlist_snapshot);
    }
    if app.workspace.is_split() {
        app.workspace.views.truncate(1);
        app.workspace.active_view_index = 0;
    }
    close_filter_popup(app);
    app.show_sort_menu = false;
    app.sort_menu_pos = None;
    app.show_floating_toolbar = false;
    app.cached_title.clear();
}

