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
            thumbnail_width: 160,
            directory_sort_preferences: std::collections::HashMap::new(),
            shortcuts: crate::shortcuts::ShortcutConfig::default(),
        }
    }
}

use crate::workspace::Workspace;

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
    pub delete_file_dialog_selection: crate::ui::dialogs::confirmation_dialog::ConfirmationSelection,
    pub show_save_overwrite_dialog: bool,
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
            delete_file_dialog_selection: crate::ui::dialogs::confirmation_dialog::ConfirmationSelection::Confirm,
            show_save_overwrite_dialog: false,
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
        handlers::rebuild_adjusted_textures(self, ctx);
        
        // 2. Render UI Layers
        ui::topbar::render(self, ctx);
        ui::filter_popup::render(self, ctx);
        ui::settings::render(self, ctx);

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

        ui::bottom_bar::render(self, ctx);

        if is_playlist_grid {
            // Playlist grid mode: render the thumbnail grid in the central panel.
            let panel_output = egui::CentralPanel::default()
                .frame(egui::Frame::new())
                .show(ctx, |ui| {
                    let bg = ui.visuals().window_fill();
                    ui.painter().rect_filled(ui.max_rect(), 0.0, bg);
                    crate::ui::playlist_grid::render(self, ctx, ui)
                });

            let grid_action = panel_output.inner;
            match grid_action {
                crate::ui::playlist_grid::PlaylistGridAction::OpenImage { path, index } => {
                    handlers::playlist_grid_open_image(self, path, index);
                }
                crate::ui::playlist_grid::PlaylistGridAction::OpenFile => {
                    self.workspace.views[self.workspace.active_view_index].browse_file_requested = true;
                }
                crate::ui::playlist_grid::PlaylistGridAction::OpenFolder => {
                    self.workspace.views[self.workspace.active_view_index].browse_folder_requested = true;
                }
                crate::ui::playlist_grid::PlaylistGridAction::None => {}
            }
        } else {
            // Canvas / Empty mode: existing rendering path.
            ui::adjustment_overlay::render(ctx, self.workspace.active_view());

            let panel_output = egui::CentralPanel::default()
                .frame(egui::Frame::new())
                .show(ctx, |ui| {
                    crate::ui::split_layout::render(self, ctx, ui, !has_modal_dialog)
                });

            let result = panel_output.inner;
            let nav_action = result.nav_action;

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

    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        let current_state = crate::persistence::PersistedAppState {
            immersive_maximized: self.settings.immersive_maximized,
            loop_playlist: self.settings.loop_playlist,
            fit_all_images_to_window: self.settings.fit_all_images_to_window,
            pixel_based_1_to_1: self.settings.pixel_based_1_to_1,
            thumbnail_width: self.settings.thumbnail_width,
            directory_sort_preferences: self.settings.directory_sort_preferences.clone(),
        };
        let _ = crate::persistence::save_persisted_state(&current_state);
    }
}