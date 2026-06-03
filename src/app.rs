use eframe::egui;
use crate::state::ViewerState;
use crate::handlers;
use crate::ui;

// --- FUTURE-PROOF CONFIGURATION ---
pub struct AppSettings {
    /// True = Top bar hides when maximized (Immersive). False = Permanent Top bar.
    pub immersive_maximized: bool, 
    pub loop_playlist: bool,
    pub fit_all_images_to_window: bool,
    pub pixel_based_1_to_1: bool,
    pub groups_enabled: bool,
    pub thumbnail_width: u32,
    pub directory_sort_preferences: std::collections::HashMap<String, crate::persistence::PersistedDirectorySortPreference>,
    pub shortcuts: crate::shortcuts::ShortcutConfig,
}
impl Default for AppSettings {
    fn default() -> Self {
        Self { 
            immersive_maximized: true,
            loop_playlist: false,
            fit_all_images_to_window: true,
            pixel_based_1_to_1: false,
            groups_enabled: false,
            thumbnail_width: 160,
            directory_sort_preferences: std::collections::HashMap::new(),
            shortcuts: crate::shortcuts::ShortcutConfig::default(),
        }
    }
}

use crate::workspace::Workspace;

#[derive(Clone)]
pub struct TransitionAnimation {
    pub is_opening: bool,
    pub start_time: f64,
    pub duration: f64,
    pub progress: f32,
    pub thumb_rect: Option<egui::Rect>,
    pub canvas_image_rect: Option<egui::Rect>,
    pub image_path: std::path::PathBuf,
    pub frames_waiting: u32,
}

pub fn compute_target_image_rect(
    ctx: &egui::Context,
    view: &ViewerState,
    playlist_grid: Option<&crate::playlist_grid::PlaylistGridState>,
    fit_all_images_to_window: bool,
    pixel_based_1_to_1: bool,
    canvas_rect: egui::Rect,
    image_path: &std::path::PathBuf,
) -> egui::Rect {
    let canvas_size = canvas_rect.size();
    if canvas_size.x <= 0.0 || canvas_size.y <= 0.0 {
        return canvas_rect;
    }

    // Check if the image resolution is known (either from loaded frames or thumbnail cache)
    let (w, h) = if let Some(res) = view.image_resolution {
        res
    } else if let Some(grid) = playlist_grid {
        if let Some(crate::playlist_grid::ThumbnailEntry::Ready { width, height, .. }) = grid.thumbnail_cache.get(image_path) {
            (*width, *height)
        } else {
            (100, 100)
        }
    } else {
        (100, 100)
    };

    if w == 0 || h == 0 {
        return canvas_rect;
    }

    let image_pixels = egui::vec2(w as f32, h as f32);
    let pixels_per_point = ctx.pixels_per_point().max(0.0001);
    let image_size = image_pixels / pixels_per_point;

    // Calculate fit scale
    let scale_w = canvas_size.x / image_size.x;
    let scale_h = canvas_size.y / image_size.y;
    let fit_scale = scale_w.min(scale_h);

    // Calculate actual scale (true size or 1:1)
    let pixel_1_to_1_scale = 1.0;
    let monitor_ppi = ctx.pixels_per_point() * 72.0;
    let true_size_scale = view.image_density
        .map(|density| (monitor_ppi / density.average_ppi()).clamp(0.01, 100.0))
        .unwrap_or(pixel_1_to_1_scale);
    let actual_scale = if pixel_based_1_to_1 {
        pixel_1_to_1_scale
    } else {
        true_size_scale
    };

    let is_small_image = fit_scale > actual_scale;
    let scale = if fit_all_images_to_window || !is_small_image {
        fit_scale
    } else {
        actual_scale
    };

    let scaled_size = image_size * scale;
    let center_offset = (canvas_size - scaled_size) / 2.0;
    let min = canvas_rect.min + center_offset;
    egui::Rect::from_min_size(min, scaled_size)
}

fn lerp_rect(r1: egui::Rect, r2: egui::Rect, t: f32) -> egui::Rect {
    egui::Rect::from_min_max(
        r1.min.lerp(r2.min, t),
        r1.max.lerp(r2.max, t),
    )
}

pub struct ImageApp {
    pub workspace: Workspace,
    pub settings: AppSettings,
    pub is_focused: bool,
    pub focus_settle_until: f64,
    #[cfg(windows)]
    pub hwnd: Option<isize>,
    
    // UI Caches
    pub cached_title: String,
    pub last_title_width: f32,

    // Track if the settings menu is open
    pub show_settings_window: bool,
    pub show_sort_menu: bool,
    pub sort_menu_pos: Option<egui::Pos2>,
    pub show_filter_popup: bool,
    pub filter_popup_focus_pending: bool,
    pub filter_popup_just_opened: bool,
    pub show_floating_toolbar: bool,
    pub split_pan_zoom_sync_enabled: bool,
    pub split_pan_zoom_sync_user_disabled: bool,
    pub show_delete_file_dialog: bool,
    pub delete_file_dialog_target: Option<std::path::PathBuf>,
    /// For playlist/group view: all paths to delete when confirmed.
    pub delete_file_dialog_targets: Vec<std::path::PathBuf>,
    pub delete_file_dialog_selection: crate::ui::dialogs::confirmation_dialog::ConfirmationSelection,
    pub show_save_overwrite_dialog: bool,
    pub show_group_assign_menu: bool,
    pub group_assign_menu_pos: Option<egui::Pos2>,
    pub show_group_assign_prompt: bool,
    pub group_assign_prompt_path: Option<std::path::PathBuf>,
    pub group_assign_prompt_source_group: u32,
    pub group_assign_target: crate::groups::GroupAssignTarget,
    pub bottom_bar_scale_editing: bool,
    pub bottom_bar_scale_input: String,
    pub bottom_bar_scale_focus_pending: bool,
    pub bottom_bar_index_editing: bool,
    pub bottom_bar_index_input: String,
    pub bottom_bar_index_focus_pending: bool,
    pub bottom_bar_edit_just_opened: bool,
    pub prev_pixel_based_1_to_1: bool,
    pub immersive_topbar_visible: bool,
    pub immersive_bottombar_visible: bool,
    pub group_drag_payload: Option<crate::groups::GroupDragPayload>,
    pub notifications: crate::notifications::NotificationToast,
    pub context_menu: crate::ui::context_menu::ContextMenuState,
    pub transition_animation: Option<TransitionAnimation>,
    pub last_central_panel_rect: egui::Rect,
    startup_open_target: Option<std::path::PathBuf>,
}

impl ImageApp {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        initial_file: Option<String>,
        persisted_state: crate::persistence::PersistedAppState,
    ) -> Self {
        
        // --- Versioning & Loading Setup ---
        let load_id = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
        let preload_epoch = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
        let scan_id = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
        let (req_tx, res_rx) = crate::image_io::spawn_image_loader(cc.egui_ctx.clone(), load_id.clone());
        let (preload_req_tx, preload_res_rx) = crate::image_io::spawn_image_loader_ordered(cc.egui_ctx.clone(), preload_epoch.clone());
        let (dir_req_tx, dir_res_rx) = crate::scanner::spawn_directory_scanner(scan_id.clone()); 
        let preload = crate::preload::PreloadRing::new(preload_epoch, preload_req_tx, preload_res_rx);
        
        let state = ViewerState::new(load_id, req_tx, res_rx, scan_id, dir_req_tx, dir_res_rx, preload);

        #[cfg(windows)]
        let hwnd = {
            use raw_window_handle::HasWindowHandle;
            let mut h = None;
            if let Ok(handle) = cc.window_handle() {
                if let raw_window_handle::RawWindowHandle::Win32(win32) = handle.as_raw() {
                    let val = win32.hwnd.get();
                    crate::win32::install_drag_subclass(val);
                    h = Some(val);
                }
            }
            h
        };

        let mut settings = AppSettings::default();
        settings.immersive_maximized = persisted_state.immersive_maximized;
        settings.loop_playlist = persisted_state.loop_playlist;
        settings.fit_all_images_to_window = persisted_state.fit_all_images_to_window;
        settings.pixel_based_1_to_1 = persisted_state.pixel_based_1_to_1;
        settings.groups_enabled = persisted_state.groups_enabled;
        settings.thumbnail_width = persisted_state.thumbnail_width;
        settings.directory_sort_preferences = persisted_state.directory_sort_preferences;
        let prev_pixel_based_1_to_1 = settings.pixel_based_1_to_1;

        let mut workspace = Workspace::new(state);
        workspace.playlist_grid = Some(crate::playlist_grid::PlaylistGridState::new(&cc.egui_ctx));
        if let Some(grid) = workspace.playlist_grid.as_mut() {
            grid.settings.thumbnail_width = settings.thumbnail_width;
        }

        let app = Self {
            workspace,
            settings,
            is_focused: true,
            focus_settle_until: 0.0,
            #[cfg(windows)]
            hwnd,
            cached_title: String::new(),
            last_title_width: 0.0,
            show_settings_window: false,
            show_sort_menu: false,
            sort_menu_pos: None,
            show_filter_popup: false,
            filter_popup_focus_pending: false,
            filter_popup_just_opened: false,
            show_floating_toolbar: false,
            split_pan_zoom_sync_enabled: false,
            split_pan_zoom_sync_user_disabled: false,
            show_delete_file_dialog: false,
            delete_file_dialog_target: None,
            delete_file_dialog_targets: Vec::new(),
            delete_file_dialog_selection: crate::ui::dialogs::confirmation_dialog::ConfirmationSelection::Confirm,
            show_save_overwrite_dialog: false,
            show_group_assign_menu: false,
            group_assign_menu_pos: None,
            show_group_assign_prompt: false,
            group_assign_prompt_path: None,
            group_assign_prompt_source_group: crate::groups::DEFAULT_GROUP_ID,
            group_assign_target: crate::groups::GroupAssignTarget::default(),
            bottom_bar_scale_editing: false,
            bottom_bar_scale_input: String::new(),
            bottom_bar_scale_focus_pending: false,
            bottom_bar_index_editing: false,
            bottom_bar_index_input: String::new(),
            bottom_bar_index_focus_pending: false,
            bottom_bar_edit_just_opened: false,
            prev_pixel_based_1_to_1,
            immersive_topbar_visible: false,
            immersive_bottombar_visible: false,
            group_drag_payload: None,
            notifications: crate::notifications::NotificationToast::new(),
            context_menu: crate::ui::context_menu::ContextMenuState::default(),
            transition_animation: None,
            last_central_panel_rect: egui::Rect::NOTHING,
            startup_open_target: initial_file.map(std::path::PathBuf::from),
        };

        app
    }
}

// --- MAIN UPDATE LOOP ---
impl eframe::App for ImageApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if let Some(path) = self.startup_open_target.take() {
            crate::handlers::open_target(self, path);
            self.workspace.content_mode = crate::workspace::ContentMode::Canvas;
            ctx.request_repaint();
        }

        // 1. Plumbing & Input
        handlers::sync_window_state(self, ctx);
        handlers::handle_drag_and_drop(self, ctx);
        handlers::handle_browse_file_request(self);
        handlers::handle_browse_folder_request(self, ctx);
        handlers::handle_keyboard(self, ctx);
        handlers::process_image_loading(self, ctx);
        handlers::process_directory_scanning(self);
        handlers::process_duplicate_scanning(self, ctx);
        if self.workspace.active_view().scanning_in_progress {
            ctx.request_repaint();
        }
        if let Some(dup_state) = self.workspace.duplicate_finder.as_ref() {
            if dup_state.any_scanning() {
                ctx.request_repaint();
            }
        }
        handlers::rebuild_adjusted_textures(self, ctx);
        handlers::process_move_animation(self, ctx);
        
        // Check for transition animation completion before rendering UI layers.
        // Doing this before canvas/grid drawing avoids a 1-frame blank gap (black frames).
        if let Some(ref mut anim) = self.transition_animation {
            if anim.start_time > 0.0 {
                let dt = ctx.input(|i| i.stable_dt).min(0.033) as f32;
                anim.progress = (anim.progress + dt / anim.duration as f32).min(1.0);

                let is_loaded = !self.workspace.active_view().frames.is_empty() || self.workspace.active_view().load_error.is_some();
                let elapsed = ctx.input(|i| i.time) - anim.start_time;
                if anim.progress >= 1.0 && (!anim.is_opening || is_loaded || elapsed >= 1.0) {
                    self.transition_animation = None;
                }
            } else if !anim.is_opening {
                if anim.frames_waiting >= 10 {
                    self.transition_animation = None;
                }
            }
        }

        // 2. Render UI Layers
        ui::topbar::render(self, ctx);
        ui::filter_popup::render(self, ctx);
        ui::settings::render(self, ctx);
        ui::group_assign_prompt::render(self, ctx);

        if self.prev_pixel_based_1_to_1 != self.settings.pixel_based_1_to_1 {
            for state in &mut self.workspace.views {
                let canvas_size = if state.last_canvas_size.x > 0.0 && state.last_canvas_size.y > 0.0 {
                    state.last_canvas_size
                } else {
                    ctx.content_rect().size()
                };
                crate::ui::canvas::reset_view_for_mode_change(
                    ctx,
                    state,
                    canvas_size,
                    self.settings.fit_all_images_to_window,
                    self.settings.pixel_based_1_to_1,
                );
            }
            self.prev_pixel_based_1_to_1 = self.settings.pixel_based_1_to_1;
            ctx.request_repaint();
        }

        let has_modal_dialog = self.show_delete_file_dialog || self.show_save_overwrite_dialog;
        
        let is_playlist_grid = self.workspace.content_mode == crate::workspace::ContentMode::PlaylistGrid;
        let is_duplicate_finder = self.workspace.content_mode == crate::workspace::ContentMode::DuplicateFinder;

        ui::bottom_bar::render(self, ctx);
        ui::notification_toast::render(self, ctx);
        ui::drag_preview::render(self, ctx);

        if is_playlist_grid || is_duplicate_finder {
            let recursive_scan_enabled = self.workspace.active_view().recursive_scan_enabled;
            let toolbar_buttons = ui::folder_toolbar::default_buttons(recursive_scan_enabled, is_duplicate_finder);
            if let Some(action) = ui::folder_toolbar::render(ctx, &toolbar_buttons) {
                match action {
                    ui::folder_toolbar::FolderToolbarAction::ToggleRecursiveScan => {
                        handlers::toggle_recursive_scan(self);
                    }
                    ui::folder_toolbar::FolderToolbarAction::FindDuplicates => {
                        handlers::toggle_duplicate_finder(self, ctx);
                    }
                }
            }

            // Renders group tabs + view (playlist grid or duplicate finder) in the central panel.
            let panel_output = egui::CentralPanel::default()
                .frame(egui::Frame::new())
                .show(ctx, |ui| {
                    let bg = ui.visuals().window_fill();
                    let panel_rect = ui.max_rect();
                    self.last_central_panel_rect = panel_rect;
                    ui.painter().rect_filled(panel_rect, 0.0, bg);

                    let tabs_height = crate::ui::group_tabs::tabs_height(self);
                    let content_rect = egui::Rect::from_min_max(
                        egui::pos2(panel_rect.min.x, panel_rect.min.y + tabs_height),
                        panel_rect.max,
                    );

                    let show_duplicate_content = is_duplicate_finder
                        && self.workspace.group_tabs.selected_id == crate::groups::DEFAULT_GROUP_ID;

                    let action_result = if show_duplicate_content {
                        let action = ui.scope_builder(egui::UiBuilder::new().max_rect(content_rect), |ui| {
                            crate::ui::duplicate_view::render(self, ctx, ui, content_rect)
                        })
                        .inner;
                        match action {
                            ui::duplicate_view::DuplicateViewAction::OpenImage { group_index, path, index_in_group } => {
                                handlers::duplicate_view_open_image(self, group_index, path, index_in_group);
                            }
                            ui::duplicate_view::DuplicateViewAction::SwitchTab(scan_type) => {
                                handlers::switch_duplicate_tab(self, scan_type);
                            }
                            ui::duplicate_view::DuplicateViewAction::ContextMenu { pos } => {
                                handlers::open_context_menu(self, pos);
                            }
                            ui::duplicate_view::DuplicateViewAction::None => {}
                        }
                        crate::ui::playlist_grid::PlaylistGridAction::None
                    } else {
                        ui.scope_builder(egui::UiBuilder::new().max_rect(content_rect), |ui| {
                            crate::ui::playlist_grid::render(self, ctx, ui)
                        })
                        .inner
                    };

                    // Render group tabs LAST so they are drawn ON TOP of the scroll/content area!
                    if tabs_height > 0.0 {
                        let tabs_rect = egui::Rect::from_min_size(
                            panel_rect.min,
                            egui::vec2(panel_rect.width(), tabs_height),
                        );
                        crate::ui::group_tabs::render_in_rect(self, ctx, ui, tabs_rect);
                    }

                    action_result
                });

            let grid_action = panel_output.inner;
            match grid_action {
                crate::ui::playlist_grid::PlaylistGridAction::OpenImage { path, index, rect } => {
                    let target_canvas_rect = if !self.workspace.is_split()
                        && self.workspace.active_view().is_fullscreen
                        && self.settings.immersive_maximized
                    {
                        ctx.content_rect()
                    } else {
                        self.last_central_panel_rect
                    };
                    let canvas_image_rect = compute_target_image_rect(
                        ctx,
                        self.workspace.active_view(),
                        self.workspace.playlist_grid.as_ref(),
                        self.settings.fit_all_images_to_window,
                        self.settings.pixel_based_1_to_1,
                        target_canvas_rect,
                        &path,
                    );
                    self.transition_animation = Some(TransitionAnimation {
                        is_opening: true,
                        start_time: ctx.input(|i| i.time),
                        duration: 0.15,
                        progress: 0.0,
                        thumb_rect: Some(rect),
                        canvas_image_rect: Some(canvas_image_rect),
                        image_path: path.clone(),
                        frames_waiting: 0,
                    });
                    handlers::playlist_grid_open_image(self, path, index);
                }
                crate::ui::playlist_grid::PlaylistGridAction::OpenFile => {
                    self.workspace.views[self.workspace.active_view_index].browse_file_requested = true;
                }
                crate::ui::playlist_grid::PlaylistGridAction::OpenFolder => {
                    self.workspace.views[self.workspace.active_view_index].browse_folder_requested = true;
                }
                crate::ui::playlist_grid::PlaylistGridAction::ContextMenu { pos } => {
                    handlers::open_context_menu(self, pos);
                }
                crate::ui::playlist_grid::PlaylistGridAction::None => {}
            }

            // Render the delete-confirmation dialog over the central panel when active.
            if self.show_delete_file_dialog {
                let panel_rect = ctx.available_rect();
                let time = ctx.input(|i| i.time);
                if let Some(action) = ui::dialogs::delete_file_dialog::render(self, ctx, panel_rect, None) {
                    match action {
                        ui::dialogs::delete_file_dialog::DeleteFileDialogAction::Cancel => {
                            handlers::cancel_delete_file_dialog(self);
                        }
                        ui::dialogs::delete_file_dialog::DeleteFileDialogAction::Confirm => {
                            let show_duplicate_content = is_duplicate_finder
                                && self.workspace.group_tabs.selected_id == crate::groups::DEFAULT_GROUP_ID;
                            if show_duplicate_content {
                                handlers::confirm_delete_file_dialog_duplicate(self, time);
                            } else {
                                handlers::confirm_delete_file_dialog_playlist(self, time);
                            }
                        }
                    }
                    ctx.request_repaint();
                }
            }
        } else {
            // Canvas / Empty mode: existing rendering path.
            ui::adjustment_overlay::render(ctx, self.workspace.active_view());

            let panel_output = egui::CentralPanel::default()
                .frame(egui::Frame::new())
                .show(ctx, |ui| {
                    self.last_central_panel_rect = ui.max_rect();
                    crate::ui::split_layout::render(self, ctx, ui, !has_modal_dialog)
                });

            let result = panel_output.inner;
            let nav_action = result.nav_action;

            // Canvas right-click → open context menu at cursor position.
            if result.context_menu_requested {
                if let Some(pos) = ctx.pointer_interact_pos() {
                    handlers::open_context_menu(self, pos);
                }
            }

            ui::floating_toolbar::render(self, ctx, result.active_canvas_rect);

            let mut dialog_backdrop_rect = result.active_canvas_rect;

            let is_single_canvas =
                self.workspace.content_mode == crate::workspace::ContentMode::Canvas && !self.workspace.is_split();
            let is_immersive = is_single_canvas
                && self.workspace.active_view().is_fullscreen
                && self.settings.immersive_maximized;
            if is_immersive {
                if self.immersive_topbar_visible {
                    dialog_backdrop_rect.min.y = dialog_backdrop_rect.min.y.max(
                        crate::ui::topbar::IMMERSIVE_TOPBAR_OVERLAY_HEIGHT,
                    );
                }
                if self.immersive_bottombar_visible {
                    dialog_backdrop_rect.max.y -= crate::ui::bottom_bar::IMMERSIVE_BOTTOM_BAR_OVERLAY_HEIGHT;
                }
                if dialog_backdrop_rect.max.y < dialog_backdrop_rect.min.y {
                    dialog_backdrop_rect.max.y = dialog_backdrop_rect.min.y;
                }
            }

            let dialog_center = if self.workspace.is_split() {
                Some(dialog_backdrop_rect.center())
            } else {
                None
            };
                
            if !has_modal_dialog {
                if let Some(direction) = nav_action {
                    handlers::navigate(self, direction);
                }
            }

            if let Some(action) = ui::dialogs::delete_file_dialog::render(self, ctx, dialog_backdrop_rect, dialog_center) {
                let time = ctx.input(|i| i.time);
                match action {
                    ui::dialogs::delete_file_dialog::DeleteFileDialogAction::Cancel => {
                        handlers::cancel_delete_file_dialog(self);
                    }
                    ui::dialogs::delete_file_dialog::DeleteFileDialogAction::Confirm => {
                        handlers::confirm_delete_file_dialog(self, time);
                    }
                }
                ctx.request_repaint();
            }

        if self.show_save_overwrite_dialog {
            let file_name = self
                .workspace
                .active_view()
                .current_file_path
                .as_ref()
                .and_then(|path| path.file_name())
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| "current file".to_string());

            let message = format!(
                "Overwrite this file with current adjustments?\n\n{}",
                file_name
            );

            let spec = ui::dialogs::confirmation_dialog::ConfirmationDialogSpec {
                id_source: "save_overwrite_confirmation_dialog",
                title: "Save File",
                message: &message,
                cancel_label: "Cancel",
                confirm_label: "Save",
            };

            if let Some(action) = ui::dialogs::confirmation_dialog::render(
                ctx,
                &spec,
                ui::dialogs::confirmation_dialog::ConfirmationSelection::Confirm,
                dialog_backdrop_rect,
                dialog_center,
            ) {
                let time = ctx.input(|i| i.time);
                match action {
                    ui::dialogs::confirmation_dialog::ConfirmationDialogAction::Cancel => {
                        handlers::cancel_save_overwrite_dialog(self);
                    }
                    ui::dialogs::confirmation_dialog::ConfirmationDialogAction::Confirm => {
                        handlers::confirm_save_overwrite_dialog(self, time);
                    }
                }
                ctx.request_repaint();
            }
            }
        } // end else (canvas mode)

        // Context menu overlay — rendered after all views, before window border.
        if let Some(action) = ui::context_menu::render(&mut self.context_menu, ctx) {
            handlers::dispatch_context_menu_action(self, ctx, &action);
        }
            
        // 3. Custom Window Border (Only when windowed)
        if !self.workspace.active_view().is_fullscreen {
            let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("window_border")));
            
            // Get the theme's high-contrast color (White in Dark mode, Black in Light mode)
            let base_color = ctx.style().visuals.strong_text_color();
            
            // Apply gamma: 100% brightness when focused, 40% when unfocused
            let stroke_color = if self.is_focused {
                base_color.gamma_multiply(0.8)
            } else {
                base_color.gamma_multiply(0.4)
            };
            
            let stroke = egui::Stroke::new(1.0, stroke_color);
            
            // Align to pixel grid for visual quality
            let mut rect = ctx.content_rect().shrink(stroke.width);
            rect.max.x -= 0.5; 
            rect.max.y -= 0.5;
            
            painter.rect_stroke(rect, 8.0, stroke, egui::StrokeKind::Inside);
        }

        // Render transition animation if active
        if self.transition_animation.is_some() {
            // First recalculate target canvas image rect dynamically when opening
            let is_opening = self.transition_animation.as_ref().unwrap().is_opening;
            if is_opening {
                let target_canvas_rect = if !self.workspace.is_split()
                    && self.workspace.active_view().is_fullscreen
                    && self.settings.immersive_maximized
                    && self.workspace.content_mode == crate::workspace::ContentMode::Canvas
                {
                    ctx.content_rect()
                } else {
                    self.last_central_panel_rect
                };
                let image_path = self.transition_animation.as_ref().unwrap().image_path.clone();
                let canvas_image_rect = compute_target_image_rect(
                    ctx,
                    self.workspace.active_view(),
                    self.workspace.playlist_grid.as_ref(),
                    self.settings.fit_all_images_to_window,
                    self.settings.pixel_based_1_to_1,
                    target_canvas_rect,
                    &image_path,
                );
                if let Some(ref mut anim) = self.transition_animation {
                    anim.canvas_image_rect = Some(canvas_image_rect);
                }
            }
        }

        if let Some(ref mut anim) = self.transition_animation {
            if anim.start_time > 0.0 {
                let t = anim.progress;
                let e = 1.0 - (1.0 - t).powi(3); // ease out cubic

                if let (Some(thumb_rect), Some(canvas_rect)) = (anim.thumb_rect, anim.canvas_image_rect) {
                    let current_rect = if anim.is_opening {
                        lerp_rect(thumb_rect, canvas_rect, e)
                    } else {
                        lerp_rect(canvas_rect, thumb_rect, e)
                    };

                    ctx.request_repaint();

                    if let Some(grid) = &self.workspace.playlist_grid {
                        if let Some(crate::playlist_grid::ThumbnailEntry::Ready { texture, .. }) = grid.thumbnail_cache.get(&anim.image_path) {
                            let painter = egui::Painter::new(
                                ctx.clone(),
                                egui::LayerId::new(egui::Order::Foreground, egui::Id::new("transition_overlay")),
                                ctx.content_rect(),
                            );

                            if anim.is_opening {
                                let bg = ctx.style().visuals.window_fill;
                                let alpha = (255.0 * e) as u8;
                                let bg_color = egui::Color32::from_rgba_unmultiplied(bg.r(), bg.g(), bg.b(), alpha);
                                painter.rect_filled(self.last_central_panel_rect, 0.0, bg_color);
                            }

                            let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
                            painter.image(texture.id(), current_rect, uv, egui::Color32::WHITE);
                        }
                    }
                }
            } else if !anim.is_opening {
                // Waiting for close layout frame
                anim.frames_waiting += 1;
                ctx.request_repaint();
            }
        }

        // Clear the group drag payload at the very end of the update loop if the mouse button was released.
        if self.group_drag_payload.is_some() && ctx.input(|i| i.pointer.any_released()) {
            self.group_drag_payload = None;
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        let current_state = crate::persistence::PersistedAppState {
            immersive_maximized: self.settings.immersive_maximized,
            loop_playlist: self.settings.loop_playlist,
            fit_all_images_to_window: self.settings.fit_all_images_to_window,
            pixel_based_1_to_1: self.settings.pixel_based_1_to_1,
            groups_enabled: self.settings.groups_enabled,
            thumbnail_width: self.settings.thumbnail_width,
            directory_sort_preferences: self.settings.directory_sort_preferences.clone(),
        };
        let _ = crate::persistence::save_persisted_state(&current_state);
    }
}