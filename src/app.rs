use eframe::egui;
use crate::state::ViewerState;
use crate::handlers;
use crate::ui;

// --- FUTURE-PROOF CONFIGURATION ---
pub struct AppSettings {
    /// True = Top bar hides when maximized (Immersive). False = Permanent Top bar.
    pub immersive_maximized: bool, 
    pub loop_playlist: bool,
}
impl Default for AppSettings {
    fn default() -> Self {
        Self { 
            immersive_maximized: true,
            loop_playlist: false
        }
    }
}

pub struct ImageApp {
    pub state: ViewerState,
    pub settings: AppSettings,
    pub is_focused: bool,
    #[cfg(windows)]
    pub hwnd: Option<isize>,
    
    // UI Caches
    pub cached_title: String,
    pub last_title_width: f32,

    // Track if the settings menu is open
    pub show_settings_window: bool,
}

impl ImageApp {
    pub fn new(cc: &eframe::CreationContext<'_>, initial_file: Option<String>) -> Self {
        
        // --- Versioning & Loading Setup ---
        let load_id = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
        let scan_id = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
        let (req_tx, res_rx) = crate::image_io::spawn_image_loader(cc.egui_ctx.clone(), load_id.clone());
        let (dir_req_tx, dir_res_rx) = crate::scanner::spawn_directory_scanner(scan_id.clone()); 
        
        let state = ViewerState::new(load_id, req_tx, res_rx, scan_id, dir_req_tx, dir_res_rx);

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

        let mut app = Self {
            state,
            settings: AppSettings::default(),
            is_focused: true,
            #[cfg(windows)]
            hwnd,
            cached_title: String::new(),
            last_title_width: 0.0,
            show_settings_window: false,
        };

        if let Some(path_str) = initial_file {
            crate::handlers::open_target(&mut app, std::path::PathBuf::from(path_str));
        }

        app
    }
}

// --- MAIN UPDATE LOOP ---
impl eframe::App for ImageApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // 1. Plumbing & Input
        handlers::sync_window_state(self, ctx);
        handlers::handle_drag_and_drop(self, ctx);
        handlers::handle_keyboard(self, ctx);
        handlers::process_image_loading(self, ctx);
        handlers::process_directory_scanning(self);
        
        // 2. Render UI Layers
        ui::topbar::render(self, ctx);
        ui::bottom_bar::render(self, ctx);
        ui::settings::render(self, ctx); 
        
        // Capture navigation actions from the canvas
        let mut nav_action = None;
        egui::CentralPanel::default()
            .frame(egui::Frame::new())
            .show(ctx, |ui| {
                // Pass the loop setting down and get the click result
                nav_action = ui::canvas::render(ctx, ui, &mut self.state, self.settings.loop_playlist);
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
}