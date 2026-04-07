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
    pub shortcuts: crate::shortcuts::ShortcutConfig,
}
impl Default for AppSettings {
    fn default() -> Self {
        Self { 
            immersive_maximized: true,
            loop_playlist: false,
            fit_all_images_to_window: true,
            pixel_based_1_to_1: false,
            shortcuts: crate::shortcuts::ShortcutConfig::default(),
        }
    }
}

pub struct ImageApp {
    pub state: ViewerState,
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
    pub bottom_bar_scale_editing: bool,
    pub bottom_bar_scale_input: String,
    pub bottom_bar_scale_focus_pending: bool,
    pub bottom_bar_index_editing: bool,
    pub bottom_bar_index_input: String,
    pub bottom_bar_index_focus_pending: bool,
    pub bottom_bar_edit_just_opened: bool,
    pub prev_pixel_based_1_to_1: bool,
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
        let prev_pixel_based_1_to_1 = settings.pixel_based_1_to_1;

        let app = Self {
            state,
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
            bottom_bar_scale_editing: false,
            bottom_bar_scale_input: String::new(),
            bottom_bar_scale_focus_pending: false,
            bottom_bar_index_editing: false,
            bottom_bar_index_input: String::new(),
            bottom_bar_index_focus_pending: false,
            bottom_bar_edit_just_opened: false,
            prev_pixel_based_1_to_1,
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
            ctx.request_repaint();
        }

        // 1. Plumbing & Input
        handlers::sync_window_state(self, ctx);
        handlers::handle_drag_and_drop(self, ctx);
        handlers::handle_keyboard(self, ctx);
        handlers::process_image_loading(self, ctx);
        handlers::process_directory_scanning(self);
        handlers::rebuild_adjusted_textures(self, ctx);
        
        // 2. Render UI Layers
        ui::topbar::render(self, ctx);
        ui::filter_popup::render(self, ctx);
        ui::settings::render(self, ctx);

        if self.prev_pixel_based_1_to_1 != self.settings.pixel_based_1_to_1 {
            let canvas_size = if self.state.last_canvas_size.x > 0.0 && self.state.last_canvas_size.y > 0.0 {
                self.state.last_canvas_size
            } else {
                ctx.content_rect().size()
            };
            crate::ui::canvas::reset_view_for_mode_change(
                ctx,
                &mut self.state,
                canvas_size,
                self.settings.fit_all_images_to_window,
                self.settings.pixel_based_1_to_1,
            );
            self.prev_pixel_based_1_to_1 = self.settings.pixel_based_1_to_1;
            ctx.request_repaint();
        }

        ui::bottom_bar::render(self, ctx);
        ui::adjustment_overlay::render(ctx, &self.state);
        
        // Capture navigation actions from the canvas
        let mut nav_action = None;
        egui::CentralPanel::default()
            .frame(egui::Frame::new())
            .show(ctx, |ui| {
                // Pass the loop setting down and get the click result
                nav_action = ui::canvas::render(
                    ctx,
                    ui,
                    &mut self.state,
                    self.settings.loop_playlist,
                    self.settings.fit_all_images_to_window,
                    self.settings.pixel_based_1_to_1,
                );
            });
            
        // Trigger navigation if an edge was clicked
        if let Some(direction) = nav_action {
            handlers::navigate(self, direction);
        }
            
        // 3. Custom Window Border (Only when windowed)
        if !self.state.is_fullscreen {
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
        };
        let _ = crate::persistence::save_persisted_state(&current_state);
    }
}