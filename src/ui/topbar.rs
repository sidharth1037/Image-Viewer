use eframe::egui::{self};
use crate::app::ImageApp;
use crate::ui::sort_controls;
use egui_phosphor::regular as icons;

const EDGE_TRIGGER_HEIGHT: f32 = 34.0;
const SORT_POPUP_ID: &str = "sort_hover_menu";

fn padded_left_row_button(ui: &mut egui::Ui, label: &str, tooltip: &str, selected: bool) -> bool {
    const H_PADDING: f32 = 10.0;

    let row_size = egui::vec2(ui.available_width(), ui.spacing().interact_size.y);
    let (rect, response) = ui.allocate_exact_size(row_size, egui::Sense::click());
    let response = response.on_hover_text(tooltip);

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

fn tooltip_with_shortcut(label: &str, shortcut: &str) -> String {
    format!("{} [{}]", label, shortcut)
}

fn is_bar_visible_in_immersive(app: &ImageApp, ctx: &egui::Context) -> bool {
    if ctx.input(|i| i.time) < app.focus_settle_until {
        return false;
    }

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
        
        // Keep enough room for window controls + settings + sort controls.
        let reserved_for_buttons: f32 = 420.0;
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

        let text_color = if app.is_focused { 
                ui.visuals().strong_text_color().gamma_multiply(0.8)
            } else {
                ui.visuals().text_color().gamma_multiply(0.8)
            };
        ui.add(egui::Label::new(egui::RichText::new(&app.cached_title).color(text_color)).selectable(false));

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_space(8.0);
            let win_up = format!("Win+{}", icons::ARROW_UP);
            let win_down = format!("Win+{}", icons::ARROW_DOWN);
            
            let close_btn = egui::Button::new(icons::X);
            let close_res = ui
                .add(close_btn)
                .on_hover_text(tooltip_with_shortcut("Close window", "Ctrl+Q"));

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
            let fullscreen_res = ui
                .button(icon)
                .on_hover_text(if app.state.is_fullscreen {
                    tooltip_with_shortcut("Restore window", &win_down)
                } else {
                    tooltip_with_shortcut("Maximize window", &win_up)
                });
            if fullscreen_res.clicked() {
                app.state.is_fullscreen = !app.state.is_fullscreen;
                ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(app.state.is_fullscreen));
            }

            let minimize_res = ui
                .button(icons::MINUS)
                .on_hover_text(tooltip_with_shortcut("Minimize window", &win_down));
            if minimize_res.clicked() {
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true)); 
            }
            
            ui.add_space(12.0); 
            
            let settings_btn = egui::Button::new(icons::GEAR_SIX).selected(app.show_settings_window);
            if ui
                .add(settings_btn)
                .on_hover_text(tooltip_with_shortcut("Settings", "Ctrl+,"))
                .clicked()
            {
                crate::handlers::toggle_settings_window(app);
            }

            ui.add_space(8.0);

            // --- Sorting Dropdown ---
            let mut sort_changed = false;
            let popup_id = egui::Id::new(SORT_POPUP_ID);
            let sort_label = sort_controls::topbar_method_label(app.state.sort_method);

            // 1. Draw the Toggle Button
            let btn_res = ui
                .button(sort_label)
                .on_hover_text(tooltip_with_shortcut("Choose sorting type", "Ctrl+Left/Right"));
            if btn_res.clicked() {
                if app.show_sort_menu {
                    app.show_sort_menu = false;
                    app.sort_menu_pos = None;
                } else {
                    app.show_sort_menu = true;
                    app.sort_menu_pos = Some(egui::pos2(btn_res.rect.left(), btn_res.rect.bottom() + 10.0));
                }
            }

            // Dedicated icon-only button for switching ascending/descending.
            let order_res = ui
                .add(egui::Button::new(sort_controls::order_icon(app.state.sort_order)))
                .on_hover_text(tooltip_with_shortcut(
                    sort_controls::order_tooltip(app.state.sort_order),
                    "Ctrl+Up/Down",
                ));
            if order_res.clicked() {
                crate::handlers::set_sort_order(app, app.state.sort_order.toggled());
            }

            let has_playlist = !app.state.playlist.is_empty();
            let can_jump_last = has_playlist && app.state.current_index + 1 < app.state.playlist.len();
            let can_jump_first = has_playlist && app.state.current_index > 0;

            let jump_last_res = ui
                .add_enabled(can_jump_last, egui::Button::new(icons::ARROW_LINE_RIGHT))
                .on_hover_text(tooltip_with_shortcut("Jump to last item", "Ctrl+Shift+J"));
            if jump_last_res.clicked() {
                crate::handlers::jump_to_playlist_edge(app, true);
            }

            let jump_first_res = ui
                .add_enabled(can_jump_first, egui::Button::new(icons::ARROW_LINE_LEFT))
                .on_hover_text(tooltip_with_shortcut("Jump to first item", "Ctrl+J"));
            if jump_first_res.clicked() {
                crate::handlers::jump_to_playlist_edge(app, false);
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
                            ui.set_width(210.0);

                            for option in sort_controls::SORT_OPTIONS {
                                let is_selected = app.state.sort_method == option.method;
                                let label = sort_controls::popup_item_label(option.method);
                                let changed = padded_left_row_button(
                                    ui,
                                    &label,
                                    &tooltip_with_shortcut("Set sorting type", "No shortcut"),
                                    is_selected,
                                );
                                if changed {
                                    app.state.sort_method = option.method;
                                    app.state.sort_order = crate::scanner::default_order_for(option.method);
                                    selected_sort_option = true;
                                }
                            }

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
                        !area_res.response.rect.contains(pos)
                            && !btn_res.rect.contains(pos)
                            && !order_res.rect.contains(pos)
                            && !jump_last_res.rect.contains(pos)
                            && !jump_first_res.rect.contains(pos)
                    });
                    
                    if clicked_outside || sort_changed {
                        app.show_sort_menu = false;
                        app.sort_menu_pos = None;
                    }
                }
            }

            // If the user picked a new method, instantly trigger a background rescan
            if sort_changed {
                crate::handlers::rescan_current_sort(app);
            }
        });
    });
}
