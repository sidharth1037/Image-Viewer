use eframe::egui;
use crate::state::ViewerState;

pub struct ImageApp {
    state: ViewerState,
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
            fn trunc_with_ext(name: &str, max_chars: usize) -> String {
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

            let shown = trunc_with_ext(&full_title, max_chars);
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

        // --- 0. Receive Data from Background Thread ---
        if let Ok(result) = self.state.res_rx.try_recv() {
            match result {
                Ok(loaded_image) => {
                    // Convert raw RGBA bytes into an egui-compatible ColorImage
                    let color_image = egui::ColorImage::from_rgba_unmultiplied(
                        [loaded_image.width as usize, loaded_image.height as usize],
                        &loaded_image.pixels,
                    );

                    // Upload the image to the GPU and store the handle
                    self.state.texture = Some(ctx.load_texture(
                        "viewer_image",
                        color_image,
                        egui::TextureOptions::LINEAR, // Smooth scaling
                    ));
                }
                Err(err) => {
                    eprintln!("Failed to load image: {}", err);
                    // (Optional) You could store this error in state to show it on screen later
                }
            }
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
        egui::CentralPanel::default()
            .frame(egui::Frame::new()) // Removes default margins
            .show(ctx, |ui| {
                // Fill the background so it's a solid color
                let rect = ui.max_rect();
                ui.painter().rect_filled(rect, 0.0, ui.visuals().window_fill());

                // Center everything inside this panel
                if let Some(texture) = &self.state.texture {
                    // 1. Allocate the entire canvas area to capture mouse inputs
                    let canvas_size = ui.available_size();
                    let (response, painter) = ui.allocate_painter(canvas_size, egui::Sense::drag());
                    
                    let image_size = texture.size_vec2();

                    // --- STEP A: Calculate Fit Scale ---
                    let scale_w = canvas_size.x / image_size.x;
                    let scale_h = canvas_size.y / image_size.y;
                    let fit_scale = scale_w.min(scale_h);

                    // --- STEP B: Enforce Auto-Fit ---
                    if self.state.auto_fit {
                        self.state.scale = fit_scale;
                        self.state.pan = egui::Vec2::ZERO;
                    }

                    // --- STEP C: Handle Zoom & Pan Inputs ---
                    
                    // 1. Zooming (Mouse Wheel)
                    if response.hovered() {
                        // THE FIX: Use egui's native smoothing instead of our custom animation loop
                        let scroll = ctx.input(|i| i.smooth_scroll_delta.y); 

                        if let Some(pointer_pos) = response.hover_pos() {
                            self.state.auto_fit = false;

                            // The continuous math perfectly scales the zoom to the speed of the wheel.
                            // Tweak this 0.005 number if you want the wheel to feel heavier or lighter.
                            let zoom_sensitivity = 0.005; 
                            let zoom_multiplier = (scroll * zoom_sensitivity).exp();

                            let old_scale = self.state.scale;
                            let mut new_scale = old_scale * zoom_multiplier;

                            // THE BOUNCE FIX: Never let the scale shrink past the window bounds
                            new_scale = new_scale.max(fit_scale);

                            // --- Zoom Towards Cursor Math ---
                            let canvas_center = response.rect.center();
                            let pointer_offset = pointer_pos - canvas_center;
                            let scale_ratio = new_scale / old_scale;
                            
                            self.state.pan -= (pointer_offset - self.state.pan) * (scale_ratio - 1.0);
                            self.state.scale = new_scale;
                        }
                    }

                    let is_zoomed_in = self.state.scale > fit_scale * 1.001;

                    // 2. Panning (Click & Drag)
                    if is_zoomed_in {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
                        
                        if response.dragged_by(egui::PointerButton::Primary) {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                            self.state.auto_fit = false;
                            self.state.pan += response.drag_delta();
                        }
                    }

                    // --- STEP D: The Clamp / Overtake ---
                    if !is_zoomed_in {
                        // Snap back to perfectly centered fit if zoomed out
                        self.state.scale = fit_scale;
                        self.state.auto_fit = true;
                        self.state.pan = egui::Vec2::ZERO;
                    } else {
                        // SMART CLAMPING
                        let scaled_size = image_size * self.state.scale;
                        let max_pan_x = ((scaled_size.x - canvas_size.x) / 2.0).max(0.0);
                        let max_pan_y = ((scaled_size.y - canvas_size.y) / 2.0).max(0.0);
                        
                        self.state.pan.x = self.state.pan.x.clamp(-max_pan_x, max_pan_x);
                        self.state.pan.y = self.state.pan.y.clamp(-max_pan_y, max_pan_y);
                    }

                    // --- STEP E: Draw the Image ---
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