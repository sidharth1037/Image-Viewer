use eframe::egui;
use crate::app::ImageApp;
use egui_phosphor::regular as icons;

const EDGE_TRIGGER_HEIGHT: f32 = 34.0;

fn is_bar_visible_in_immersive(app: &ImageApp, ctx: &egui::Context) -> bool {
    let bottom_trigger = ctx.content_rect().max.y - EDGE_TRIGGER_HEIGHT;

    match ctx.pointer_hover_pos() {
        Some(pos) => pos.y < EDGE_TRIGGER_HEIGHT || pos.y >= bottom_trigger,
        None => {
            #[cfg(windows)]
            {
                app.is_focused && app.hwnd.is_some_and(|h| {
                    let y = crate::win32::get_cursor_client_y(h) as f32;
                    (y >= 0.0 && y < EDGE_TRIGGER_HEIGHT) || y >= bottom_trigger
                })
            }
            #[cfg(not(windows))]
            {
                false
            }
        }
    }
}

pub fn render(app: &mut ImageApp, ctx: &egui::Context) {
    // --- SETTINGS INTEGRATION ---
    let is_immersive = app.state.is_fullscreen && app.settings.immersive_maximized;

    if is_immersive {
        let mut show_bars = is_bar_visible_in_immersive(app, ctx);

        // --- THE FIX: KEEP OPEN IF DROPDOWN IS ACTIVE ---
        // If the user has clicked the combo box, egui registers a popup.
        // We force the top bar to stay visible until they click away.
        if egui::Popup::is_any_open(ctx) {
            show_bars = true;
        }

        if show_bars {
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
            ctx.style().visuals.strong_text_color().gamma_multiply(0.4)
        };

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
        
        // --- THE FIX: PREVENT TITLE OVERLAP ---
        // Increased from 120.0 to 280.0 to account for the ComboBox and Settings button
        let reserved_for_buttons: f32 = 280.0; 
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
            
            let close_btn = egui::Button::new(icons::X);
            let close_res = ui.add(close_btn);

            if close_res.hovered() {
                ui.painter().rect_filled(
                    close_res.rect, 
                    ui.style().visuals.widgets.hovered.corner_radius, 
                    egui::Color32::from_rgb(210, 43, 43)
                );
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
            
            let icon = if app.state.is_fullscreen { icons::CORNERS_IN } else { icons::CORNERS_OUT };
            if ui.button(icon).clicked() {
                app.state.is_fullscreen = !app.state.is_fullscreen;
                ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(app.state.is_fullscreen));
            }

            if ui.button(icons::MINUS).clicked() { 
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true)); 
            }
            
            ui.add_space(12.0); 
            
            let settings_btn = egui::Button::new(icons::GEAR_SIX).selected(app.show_settings_window);
            if ui.add(settings_btn).clicked() {
                app.show_settings_window = !app.show_settings_window; 
            }

            ui.add_space(8.0);

            // --- Sorting Dropdown ---
            let mut sort_changed = false;
            
            let sort_label = match app.state.sort_method {
                crate::scanner::SortMethod::Alphabetical => format!("{} Alphabetical", icons::SORT_ASCENDING),
                crate::scanner::SortMethod::Natural => format!("{} Natural", icons::SORT_ASCENDING),
                crate::scanner::SortMethod::Size => format!("{} Size", icons::ARROWS_DOWN_UP),
                crate::scanner::SortMethod::DateModified => format!("{} Modified", icons::CLOCK),
                crate::scanner::SortMethod::DateCreated => format!("{} Created", icons::CALENDAR_PLUS),
            };

            // Style the popup locally to avoid global context style churn every frame.
            ui.scope(|ui| {
                let style = ui.style_mut();
                style.visuals.window_fill = egui::Color32::TRANSPARENT;
                style.visuals.window_stroke = egui::Stroke::NONE;
                style.visuals.popup_shadow = egui::epaint::Shadow::NONE;
                style.spacing.menu_margin = egui::Margin::same(0);

                egui::ComboBox::from_id_salt("sort_combo_box")
                    .selected_text(sort_label)
                    .width(110.0)
                    .show_ui(ui, |ui| {
                        ui.add_space(14.0);

                        egui::Frame::menu(ui.style()).show(ui, |ui| {
                            sort_changed |= ui.selectable_value(&mut app.state.sort_method, crate::scanner::SortMethod::Alphabetical, "Name (Alphabetical)").changed();
                            sort_changed |= ui.selectable_value(&mut app.state.sort_method, crate::scanner::SortMethod::Natural, "Name (Natural)").changed();
                            sort_changed |= ui.selectable_value(&mut app.state.sort_method, crate::scanner::SortMethod::Size, "Size").changed();
                            sort_changed |= ui.selectable_value(&mut app.state.sort_method, crate::scanner::SortMethod::DateModified, "Date Modified").changed();
                            sort_changed |= ui.selectable_value(&mut app.state.sort_method, crate::scanner::SortMethod::DateCreated, "Date Created").changed();
                        });
                    });
            });

            // If the user picked a new method, instantly trigger a background rescan
            if sort_changed {
                // Determine the target path safely, even if the playlist is currently empty
                let target = if !app.state.playlist.is_empty() {
                    Some(app.state.playlist[app.state.current_index].clone())
                } else if let Some(folder) = &app.state.current_folder {
                    if !app.state.current_file_name.is_empty() {
                        Some(folder.join(&app.state.current_file_name))
                    } else {
                        None
                    }
                } else {
                    None
                };

                // Send the new sort request
                if let Some(path) = target {
                    crate::handlers::request_directory_scan(app, path);
                }
            }
        });
    });
}