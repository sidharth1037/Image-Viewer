use eframe::egui;
use crate::state::ViewerState;

pub struct ImageApp {
    state: ViewerState,
    #[cfg(windows)]
    hwnd: Option<isize>,
}

impl ImageApp {
    pub fn new(cc: &eframe::CreationContext<'_>, initial_file: Option<String>) -> Self {

        let mut state = ViewerState::new();

        if let Some(path) = initial_file {
            // For now, just extract the file name to show in the title bar
            if let Some(name) = std::path::Path::new(&path).file_name() {
                state.current_file_name = name.to_string_lossy().into_owned();
            }
        }

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

        Self {
            state,
            #[cfg(windows)]
            hwnd,
        }
    }

    // Helper function to draw the actual buttons and handle dragging
    fn render_top_bar_content(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        // 1. Make the whole bar area draggable to move the window
        let drag_response = ui.interact(ui.max_rect(), egui::Id::new("title_drag"), egui::Sense::drag());
        if drag_response.drag_started() {
            ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }

        ui.horizontal_centered(|ui| {
            ui.add_space(8.0);
            // compute raw title
            let full_title = if self.state.current_file_name.is_empty() {
                "Image Viewer".to_string()
            } else {
                self.state.current_file_name.clone()
            };

            // estimate reserved space for the right-side buttons (tweak if you change icons/layout)
            let reserved_for_buttons: f32 = 120.0; // px

            // available width for the title (never negative)
            let avail_px = (ui.available_width() - reserved_for_buttons).max(0.0);

            // average character width in your UI font (tweak for better fit)
            let avg_char_px = 7.0_f32;

            // compute max chars that can fit in available width
            let max_chars = (avail_px / avg_char_px).floor() as usize;

            // helper: keeps extension and returns original if it already fits
            fn elide_keep_ext(name: &str, max_chars: usize) -> String {
                if name.len() <= max_chars || max_chars < 5 {
                    return name.to_string();
                }
                let path = std::path::Path::new(name);
                let ext = path.extension().and_then(|s| s.to_str()).map(|s| format!(".{}", s));
                let ext_len = ext.as_ref().map(|s| s.len()).unwrap_or(0);
                let prefix_len = max_chars.saturating_sub(ext_len + 3).max(1);
                let prefix: String = name.chars().take(prefix_len).collect();
                match ext {
                    Some(e) => format!("{}..{}", prefix, e),
                    None => format!("{}...", prefix),
                }
            }

            let shown = elide_keep_ext(&full_title, max_chars);
            ui.label(shown);

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(8.0);

                // Close Button
                if ui.button("❌").clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }

                // Immersive Toggle (Maximize + Hide Bar)
                let icon = if self.state.is_fullscreen { "🗖" } else { "🗗" };
                if ui.button(icon).clicked() {
                    self.state.is_fullscreen = !self.state.is_fullscreen;
                    // Maximize window when immersive, restore when windowed
                    ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(self.state.is_fullscreen));
                }

                // Minimize Button (Only visible if not immersive)
                if ui.button("🗕").clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                }
            });
        });
    }
}

impl eframe::App for ImageApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {

        // 1. Ask the OS if the window is currently maximized
        let is_currently_maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));

        // 2. Sync: If OS is maximized but our state is FALSE, set it to TRUE
        // 3. Sync: If OS is NOT maximized but our state is TRUE, set it to FALSE
        if is_currently_maximized != self.state.is_fullscreen {
            self.state.is_fullscreen = is_currently_maximized;
        }

        // --- 1. Top Bar Rendering ---
        if self.state.is_fullscreen {
            // Immersive Mode: Only show as an overlay when mouse touches top
            let near_top = match ctx.pointer_hover_pos() {
                Some(pos) => pos.y < 34.0,
                None => {
                    // egui may not see the cursor when it's in the native HTCAPTION zone.
                    // Fall back to querying the OS directly, but only if we have focus
                    // (otherwise the cursor is outside the window entirely).
                    #[cfg(windows)]
                    {
                        let has_focus = ctx.input(|i| i.viewport().focused.unwrap_or(false));
                        has_focus && self.hwnd.is_some_and(|h| {
                            let y = crate::win32::get_cursor_client_y(h);
                            y >= 0 && y < 34
                        })
                    }
                    #[cfg(not(windows))]
                    false
                }
            };
            if near_top {
                egui::Area::new(egui::Id::new("top_bar_overlay"))
                    .fixed_pos(egui::pos2(0.0, 0.0))
                    .order(egui::Order::Foreground)
                    .show(ctx, |ui| {
                        ui.set_width(ctx.content_rect().width());
                        egui::Frame::menu(ui.style()).show(ui, |ui| {
                            ui.set_height(22.0);
                            self.render_top_bar_content(ui, ctx);
                        });
                    });
            }
        } else {
            // Normal Mode: Fixed panel that pushes content down
            egui::TopBottomPanel::top(egui::Id::new("custom_title_bar"))
                .exact_height(32.0)
                .show(ctx, |ui| {
                    self.render_top_bar_content(ui, ctx);
                });
        }

        // --- 2. Main Canvas ---
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(20.0);
                if self.state.is_fullscreen {
                    ui.label("IMMERSE MODE: Hover at the top to see the bar.");
                } else {
                    ui.label("WINDOW MODE: The bar is fixed.");
                }
            });
        });

        // --- 3. Window Border (windowed mode only) ---
        if !self.state.is_fullscreen {
            let painter = ctx.layer_painter(egui::LayerId::new(
                egui::Order::Foreground,
                egui::Id::new("window_border"),
            ));
            let stroke = ctx.style().visuals.window_stroke;
            let rect = ctx.content_rect().shrink(stroke.width * 0.5);
            painter.rect_stroke(rect, 8.0, stroke, egui::StrokeKind::Inside);
        }
    }
}