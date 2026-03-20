use eframe::egui;
use crate::state::ViewerState;

pub struct ImageApp {
    state: ViewerState,
    is_focused: bool,
    #[cfg(windows)]
    hwnd: Option<isize>,
}

impl ImageApp {
    pub fn new(cc: &eframe::CreationContext<'_>, initial_file: Option<String>) -> Self {
        
        // 1. Pass the context cloned from cc to the loader
        let (req_tx, res_rx) = crate::image_io::spawn_image_loader(cc.egui_ctx.clone());
        let mut state = ViewerState::new(req_tx, res_rx);

        // 3. Handle the command line argument if it exists
        if let Some(path_str) = initial_file {
            let path = std::path::PathBuf::from(&path_str);
            
            // Extract the file name for the title bar
            if let Some(name) = path.file_name() {
                state.current_file_name = name.to_string_lossy().into_owned();
            }
            
            // Send the path to the background thread to start loading immediately
            let _ = state.req_tx.send(path);
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
            is_focused: true,
            #[cfg(windows)]
            hwnd,
        }
    }

    // --- HELPER: Window Syncing ---
    fn sync_window_state(&mut self, ctx: &egui::Context) {
        let is_maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
        if is_maximized != self.state.is_fullscreen {
            self.state.is_fullscreen = is_maximized;
        }

        if let Some(focused) = ctx.input(|i| i.viewport().focused) {
            self.is_focused = focused;
        }
    }

    // --- HELPER: Background Image Loading ---
    fn process_image_loading(&mut self, ctx: &egui::Context) {
        if let Ok(result) = self.state.res_rx.try_recv() {
            if let Ok(loaded_image) = result {
                let color_image = egui::ColorImage::from_rgba_unmultiplied(
                    [loaded_image.width as usize, loaded_image.height as usize],
                    &loaded_image.pixels,
                );
                self.state.texture = Some(ctx.load_texture(
                    "viewer_image",
                    color_image,
                    egui::TextureOptions::LINEAR,
                ));
            } else if let Err(err) = result {
                eprintln!("Failed to load image: {}", err);
            }
        }
    }

    // --- HELPER: Top Bar & Controls ---
    fn render_top_bar(&mut self, ctx: &egui::Context) {
        if self.state.is_fullscreen {
            let near_top = match ctx.pointer_hover_pos() {
                Some(pos) => pos.y < 34.0,
                None => {
                    #[cfg(windows)]
                    {
                        self.is_focused && self.hwnd.is_some_and(|h| {
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

                        // --- THE FIX: High-Contrast Active Border ---
                        // We create a custom frame that uses the theme's strongest text color for the border.
                        // This automatically switches between bright white (Dark Mode) and dark black (Light Mode).
                        let active_stroke = egui::Stroke::new(
                            1.0, 
                            ui.visuals().strong_text_color().gamma_multiply(0.8)
                        );
                        
                        let custom_frame = egui::Frame::menu(ui.style())
                            .stroke(active_stroke);

                        custom_frame.show(ui, |ui| {
                            ui.set_height(22.0);
                            self.render_top_bar_content(ui, ctx);
                        });
                    });
            }
        } else {
            // Instant color swap for the top line
            let current_color = if self.is_focused {
                ctx.style().visuals.strong_text_color().gamma_multiply(0.8)
            } else {
                ctx.style().visuals.window_stroke.color.gamma_multiply(0.4)
            };
            let panel_stroke = egui::Stroke::new(1.0, current_color);

            let panel_frame = egui::Frame::side_top_panel(&ctx.style())
                .inner_margin(egui::Margin::same(0))
                .stroke(egui::Stroke::NONE);

            let panel_res = egui::TopBottomPanel::top(egui::Id::new("custom_title_bar"))
                .frame(panel_frame) 
                .show_separator_line(false) 
                .exact_height(32.0)
                .show(ctx, |ui| {
                    self.render_top_bar_content(ui, ctx);
                });

            let rect = panel_res.response.rect;
            ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("title_line")))
                .hline(rect.x_range(), rect.bottom(), panel_stroke);
        }
    }

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
            fn trunc_with_ext(name: &str, max_chars: usize) -> String {
                if name.len() <= max_chars || max_chars < 5 { return name.to_string(); }
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

            let shown = trunc_with_ext(&full_title, max_chars);
            
            // Instant color swap for the text
            let text_color = if self.is_focused {
                ui.visuals().strong_text_color()
            } else {
                ui.visuals().text_color().gamma_multiply(0.8)
            };

            ui.add(egui::Label::new(egui::RichText::new(shown).color(text_color)).selectable(false));

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(8.0);
                if ui.button("❌").clicked() { ctx.send_viewport_cmd(egui::ViewportCommand::Close); }
                
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

    // --- HELPER: Image Canvas & Math ---
    fn render_main_canvas(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default()
            .frame(egui::Frame::new()) // Removes default margins
            .show(ctx, |ui| {
                // Fill the background so it's a solid color
                let rect = ui.max_rect();
                ui.painter().rect_filled(rect, 0.0, ui.visuals().window_fill());

                // Center everything inside this panel
                if let Some(texture) = &self.state.texture {
                    // Allocate the entire canvas area to capture mouse inputs
                    let canvas_size = ui.available_size();
                    let (response, painter) = ui.allocate_painter(canvas_size, egui::Sense::click_and_drag());
                    let image_size = texture.size_vec2();

                    // Calculate Fit Scale
                    let scale_w = canvas_size.x / image_size.x;
                    let scale_h = canvas_size.y / image_size.y;
                    let fit_scale = scale_w.min(scale_h);

                    // Enforce Auto-Fit
                    if self.state.auto_fit {
                        self.state.scale = fit_scale;
                        self.state.pan = egui::Vec2::ZERO;
                    }

                    // Handle Zoom & Pan Inputs
                    if response.hovered() {
                        let scroll = ctx.input(|i| i.smooth_scroll_delta.y); 
                        if let Some(pointer_pos) = response.hover_pos() {
                            self.state.auto_fit = false;

                            // The continuous math perfectly scales the zoom to the speed of the wheel.
                            // Tweak this 0.005 number if you want the wheel to feel heavier or lighter.
                            let zoom_multiplier = (scroll * 0.005).exp();
                            let old_scale = self.state.scale;
                            // THE BOUNCE FIX: Never let the scale shrink past the window bounds
                            let new_scale = (old_scale * zoom_multiplier).max(fit_scale);

                            // Zoom Towards Cursor Math
                            let canvas_center = response.rect.center();
                            let pointer_offset = pointer_pos - canvas_center;
                            let scale_ratio = new_scale / old_scale;
                            
                            self.state.pan -= (pointer_offset - self.state.pan) * (scale_ratio - 1.0);
                            self.state.scale = new_scale;
                        }
                    }

                    let is_zoomed_in = self.state.scale > fit_scale * 1.001;

                    // Panning (Click & Drag)
                    if is_zoomed_in {
                        if response.dragged_by(egui::PointerButton::Primary) {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                            self.state.auto_fit = false;
                            self.state.pan += response.drag_delta();
                        }
                        
                        // The Clamp / Overtake Fix: When zoomed in, allow panning just a bit beyond the edges for a more natural feel
                        // But then clamp it so you can never pan the image completely out of view.
                        let scaled_size = image_size * self.state.scale;
                        let max_pan_x = ((scaled_size.x - canvas_size.x) / 2.0).max(0.0);
                        let max_pan_y = ((scaled_size.y - canvas_size.y) / 2.0).max(0.0);
                        
                        self.state.pan.x = self.state.pan.x.clamp(-max_pan_x, max_pan_x);
                        self.state.pan.y = self.state.pan.y.clamp(-max_pan_y, max_pan_y);
                    } else {
                        self.state.scale = fit_scale;
                        self.state.auto_fit = true;
                        self.state.pan = egui::Vec2::ZERO;
                    }

                    // Draw the Image
                    let scaled_size = image_size * self.state.scale;
                    let center_offset = (canvas_size - scaled_size) / 2.0;
                    let image_top_left = response.rect.min + center_offset + self.state.pan;
                    let draw_rect = egui::Rect::from_min_size(image_top_left, scaled_size);
                    let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
                    
                    painter.image(texture.id(), draw_rect, uv, egui::Color32::WHITE);
                } else {
                    // If no texture is loaded yet, show what's happening
                    ui.centered_and_justified(|ui| {
                        if self.state.current_file_name.is_empty() {
                            ui.label("No image loaded. Try 'Open With'...");
                        } else {
                            ui.spinner();
                        }
                    });
                }
            });
    }

    // --- HELPER: Window Border ---
    fn render_window_border(&mut self, ctx: &egui::Context) {
        if !self.state.is_fullscreen {
            let painter = ctx.layer_painter(egui::LayerId::new(
                egui::Order::Foreground,
                egui::Id::new("window_border"),
            ));
            
            let mut stroke = ctx.style().visuals.window_stroke;
            
            if self.is_focused {
                stroke.color = ctx.style().visuals.strong_text_color().gamma_multiply(0.8);
            } else {
                stroke.color = ctx.style().visuals.window_stroke.color.gamma_multiply(0.4);
            }

            // 1. Get your preferred rectangle
            let mut rect = ctx.content_rect().shrink(stroke.width);

            // 2. THE FIX: Compensate for Windows DWM clipping.
            // Pull the bottom and right edges inward by exactly 1 pixel so 
            // the OS rounding mask doesn't shave off the thickness.
            rect.max.x -= 0.5;
            rect.max.y -= 0.5;

            painter.rect_stroke(rect, 8.0, stroke, egui::StrokeKind::Inside);
        }
    }
}

// --- MAIN UPDATE LOOP ---
impl eframe::App for ImageApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.sync_window_state(ctx);
        self.process_image_loading(ctx);
        
        self.render_top_bar(ctx);
        self.render_main_canvas(ctx);
        self.render_window_border(ctx);
    }
}