use eframe::egui::{self};
use crate::app::ImageApp;
use egui_phosphor::regular as icons;

const EDGE_TRIGGER_HEIGHT: f32 = 34.0;
const SORT_POPUP_ID: &str = "sort_hover_menu";

fn padded_left_row_button(ui: &mut egui::Ui, label: &str, selected: bool) -> bool {
    const H_PADDING: f32 = 10.0;

    let row_size = egui::vec2(ui.available_width(), ui.spacing().interact_size.y);
    let (rect, response) = ui.allocate_exact_size(row_size, egui::Sense::click());

    if ui.is_rect_visible(rect) {
        let visuals = &ui.style().visuals;
        let widget = if selected {
            &visuals.widgets.active
        } else if response.hovered() {
            &visuals.widgets.hovered
        } else {
            &visuals.widgets.inactive
        };

        ui.painter().rect_filled(rect, widget.corner_radius, widget.bg_fill);

        ui.painter().text(
            egui::pos2(rect.left() + H_PADDING, rect.center().y),
            egui::Align2::LEFT_CENTER,
            label,
            egui::TextStyle::Button.resolve(ui.style()),
            widget.fg_stroke.color,
        );
    }

    response.clicked()
}

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
        // We force the top bar to stay visible if our custom hover menu is open.
        if app.show_sort_menu {
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
        
        // Z-INDEX FIX: Render line on Middle order so it stays cleanly under popups
        ctx.layer_painter(egui::LayerId::new(egui::Order::Middle, egui::Id::new("title_line")))
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

            let popup_id = egui::Id::new(SORT_POPUP_ID);

            // 1. Draw the Toggle Button
            let btn_res = ui.button(sort_label);
            if btn_res.clicked() {
                if app.show_sort_menu {
                    app.show_sort_menu = false;
                    app.sort_menu_pos = None;
                } else {
                    app.show_sort_menu = true;
                    app.sort_menu_pos = Some(egui::pos2(btn_res.rect.left(), btn_res.rect.bottom() + 10.0));
                }
            }

            // 2. Draw the Hovering Menu Area if open
            if app.show_sort_menu {
                let popup_pos = app
                    .sort_menu_pos
                    .unwrap_or_else(|| egui::pos2(btn_res.rect.left(), btn_res.rect.bottom() + 10.0));

                let mut selected_sort_option = false;

                let area_res = egui::Area::new(popup_id)
                    // Keep a visible gap so it feels detached from the trigger button.
                    .fixed_pos(popup_pos)
                    .order(egui::Order::Tooltip)
                    .show(ctx, |ui| {
                        // Use a native menu frame for the background
                        egui::Frame::menu(ui.style()).show(ui, |ui| {
                            // FIXED: Force a strict width. Because Area is an unconstrained floating 
                            // container, `available_width()` otherwise extends to the edge of the screen!
                            ui.set_width(170.0);

                            let alpha_selected = app.state.sort_method == crate::scanner::SortMethod::Alphabetical;
                            let changed_alpha = padded_left_row_button(ui, "Name (Alphabetical)", alpha_selected);
                            if changed_alpha {
                                app.state.sort_method = crate::scanner::SortMethod::Alphabetical;
                            }

                            let natural_selected = app.state.sort_method == crate::scanner::SortMethod::Natural;
                            let changed_natural = padded_left_row_button(ui, "Name (Natural)", natural_selected);
                            if changed_natural {
                                app.state.sort_method = crate::scanner::SortMethod::Natural;
                            }

                            let size_selected = app.state.sort_method == crate::scanner::SortMethod::Size;
                            let changed_size = padded_left_row_button(ui, "Size", size_selected);
                            if changed_size {
                                app.state.sort_method = crate::scanner::SortMethod::Size;
                            }

                            let modified_selected = app.state.sort_method == crate::scanner::SortMethod::DateModified;
                            let changed_modified = padded_left_row_button(ui, "Date Modified", modified_selected);
                            if changed_modified {
                                app.state.sort_method = crate::scanner::SortMethod::DateModified;
                            }

                            let created_selected = app.state.sort_method == crate::scanner::SortMethod::DateCreated;
                            let changed_created = padded_left_row_button(ui, "Date Created", created_selected);
                            if changed_created {
                                app.state.sort_method = crate::scanner::SortMethod::DateCreated;
                            }

                            selected_sort_option = changed_alpha
                                || changed_natural
                                || changed_size
                                || changed_modified
                                || changed_created;

                            sort_changed |= selected_sort_option;
                        });
                    });

                if selected_sort_option {
                    app.show_sort_menu = false;
                    app.sort_menu_pos = None;
                }

                // 3. Close popup logic if clicked outside
                if app.show_sort_menu && ctx.input(|i| i.pointer.any_pressed()) {
                    let clicked_outside = ctx.pointer_interact_pos().map_or(false, |pos| {
                        !area_res.response.rect.contains(pos) && !btn_res.rect.contains(pos)
                    });
                    
                    if clicked_outside || sort_changed {
                        app.show_sort_menu = false;
                        app.sort_menu_pos = None;
                    }
                }
            }

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