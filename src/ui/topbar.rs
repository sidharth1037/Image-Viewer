use eframe::egui;
use crate::app::ImageApp;
use egui_phosphor::regular as icons;

pub fn render(app: &mut ImageApp, ctx: &egui::Context) {
    // --- SETTINGS INTEGRATION ---
    // Should we auto-hide the bar, or keep it permanently visible?
    let is_immersive = app.state.is_fullscreen && app.settings.immersive_maximized;

    if is_immersive {
        let near_top = match ctx.pointer_hover_pos() {
            Some(pos) => pos.y < 34.0,
            None => {
                #[cfg(windows)]
                {
                    app.is_focused && app.hwnd.is_some_and(|h| {
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
                    let active_stroke = egui::Stroke::new(1.0, ui.visuals().strong_text_color().gamma_multiply(0.8));
                    
                    egui::Frame::menu(ui.style()).stroke(active_stroke).show(ui, |ui| {
                        ui.set_height(22.0);
                        render_content(app, ui, ctx);
                    });
                });
        }
    } else {
        // Standard Permanent Top Bar
        let mut current_color = if app.is_focused {
            ctx.style().visuals.strong_text_color().gamma_multiply(0.8)
        } else {
            ctx.style().visuals.window_stroke.color.gamma_multiply(0.4)
        };

        // --- THE FIX: Dim the line when maximized instead of hiding it ---
        if app.state.is_fullscreen {
            current_color = current_color.gamma_multiply(0.4);
        }

        let panel_stroke = egui::Stroke::new(1.0, current_color);

        let panel_frame = egui::Frame::side_top_panel(&ctx.style())
            .inner_margin(egui::Margin::same(0))
            .stroke(egui::Stroke::NONE);

        let panel_res = egui::TopBottomPanel::top(egui::Id::new("custom_title_bar"))
            .frame(panel_frame) 
            .show_separator_line(false) 
            .exact_height(32.0)
            .show(ctx, |ui| {
                render_content(app, ui, ctx);
            });

        // Draw it unconditionally now, trusting our dimming math above
        let rect = panel_res.response.rect;
        ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("title_line")))
            .hline(rect.x_range(), rect.bottom(), panel_stroke);
    }
}

fn render_content(app: &mut ImageApp, ui: &mut egui::Ui, ctx: &egui::Context) {
    let drag_response = ui.interact(ui.max_rect(), egui::Id::new("title_drag"), egui::Sense::drag());
    if drag_response.drag_started() {
        ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
    }

    ui.horizontal_centered(|ui| {
        ui.add_space(8.0);
        
        let reserved_for_buttons: f32 = 120.0; 
        let avail_px = (ui.available_width() - reserved_for_buttons).max(0.0);

        if (app.last_title_width - avail_px).abs() > 5.0 || app.cached_title.is_empty() {
            let full_title = if app.state.current_file_name.is_empty() { 
                "Image Viewer".to_string() 
            } else { 
                app.state.current_file_name.clone() 
            };
            
            let max_chars = (avail_px / 7.0).floor() as usize;

            app.cached_title = if full_title.len() <= max_chars || max_chars < 5 { 
                full_title 
            } else {
                let path = std::path::Path::new(&full_title);
                let ext = path.extension().and_then(|s| s.to_str()).map(|s| format!(".{}", s));
                let ext_len = ext.as_ref().map(|s| s.len()).unwrap_or(0);
                let prefix_len = max_chars.saturating_sub(ext_len + 3).max(1);
                let prefix: String = full_title.chars().take(prefix_len).collect();
                match ext {
                    Some(e) => format!("{}..{}", prefix, e),
                    None => format!("{}...", prefix),
                }
            };
            app.last_title_width = avail_px;
        }

        let text_color = if app.is_focused { ui.visuals().strong_text_color() } else { ui.visuals().text_color().gamma_multiply(0.8) };
        ui.add(egui::Label::new(egui::RichText::new(&app.cached_title).color(text_color)).selectable(false));

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_space(8.0);
            // --- Close Button with Custom Red Hover ---
            let close_btn = egui::Button::new(icons::X);
            let close_res = ui.add(close_btn);

            if close_res.hovered() {
                // Apply a muted red background when hovered
                // Color32::from_rgb(180, 50, 50) is a "not too bright" professional red
                ui.painter().rect_filled(
                    close_res.rect, 
                    ui.style().visuals.widgets.hovered.corner_radius, 
                    egui::Color32::from_rgb(210, 43, 43)
                );
                // Re-paint the icon on top of the red background so it remains visible
                ui.painter().text(
                    close_res.rect.center(),
                    egui::Align2::CENTER_CENTER,
                    icons::X,
                    egui::FontId::proportional(14.0),
                    ui.visuals().strong_text_color(),
                );
            }

            if close_res.clicked() {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            
            // Maximize/Restore Toggle
            let icon = if app.state.is_fullscreen { icons::CORNERS_IN } else { icons::CORNERS_OUT };
            if ui.button(icon).clicked() {
                app.state.is_fullscreen = !app.state.is_fullscreen;
                ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(app.state.is_fullscreen));
            }

            // Minimize Button
            if ui.button(icons::MINUS).clicked() { 
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true)); 
            }
            
            ui.add_space(12.0); 
            
            // Settings Button using Phosphor Gear
            let settings_btn = egui::Button::new(icons::GEAR_SIX).selected(app.show_settings_window);
            if ui.add(settings_btn).clicked() {
                app.show_settings_window = !app.show_settings_window; 
            }
        });
    });
}