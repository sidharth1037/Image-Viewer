use crate::app::ImageApp;
use eframe::egui;

const BOTTOM_BAR_HEIGHT: f32 = 28.0;
pub const IMMERSIVE_BOTTOM_BAR_OVERLAY_HEIGHT: f32 = BOTTOM_BAR_HEIGHT;
const EDGE_TRIGGER_HEIGHT: f32 = 34.0;
const SCALE_INPUT_ID: &str = "bottom_bar_scale_input";
const INDEX_INPUT_ID: &str = "bottom_bar_index_input";

fn format_file_size(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = 1024.0 * 1024.0;
    const GB: f64 = 1024.0 * 1024.0 * 1024.0;

    let b = bytes as f64;
    if b >= GB {
        format!("{:.2} GB", b / GB)
    } else if b >= MB {
        format!("{:.2} MB", b / MB)
    } else if b >= KB {
        format!("{:.1} KB", b / KB)
    } else {
        format!("{} B", bytes)
    }
}

fn is_bottom_visible_in_immersive(app: &ImageApp, ctx: &egui::Context) -> bool {
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

fn text_like_button(ui: &mut egui::Ui, text: &str) -> egui::Response {
    let mut button_style: egui::Style = ui.style().as_ref().clone();
    button_style.spacing.button_padding = egui::vec2(4.0, 1.0);
    button_style.visuals.widgets.inactive.bg_fill = egui::Color32::TRANSPARENT;
    button_style.visuals.widgets.inactive.weak_bg_fill = egui::Color32::TRANSPARENT;
    button_style.visuals.widgets.inactive.bg_stroke = egui::Stroke::NONE;
    button_style.visuals.widgets.inactive.fg_stroke.color = ui.visuals().text_color();
    button_style.visuals.widgets.hovered.bg_fill =
        ui.visuals().widgets.hovered.weak_bg_fill.gamma_multiply(1.25);
    button_style.visuals.widgets.hovered.weak_bg_fill =
        ui.visuals().widgets.hovered.weak_bg_fill.gamma_multiply(1.25);
    button_style.visuals.widgets.hovered.bg_stroke = egui::Stroke::NONE;
    button_style.visuals.widgets.hovered.fg_stroke.color = ui.visuals().text_color();
    button_style.visuals.widgets.active = button_style.visuals.widgets.hovered;

    let response = ui.scope(|ui| {
        ui.set_style(button_style);
        ui.add(
            egui::Button::new(egui::RichText::new(text).text_style(egui::TextStyle::Body))
                .frame(true)
                .min_size(egui::vec2(0.0, 0.0)),
        )
    });

    if response.inner.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Default);
    }

    response.inner
}

fn scale_bounds_percent(app: &ImageApp, ctx: &egui::Context) -> (f32, f32, f32, f32, f32) {
    let active_view = app.workspace.active_view();
    let canvas_size = if active_view.last_canvas_size.x > 0.0 && active_view.last_canvas_size.y > 0.0 {
        active_view.last_canvas_size
    } else {
        ctx.content_rect().size()
    };

    if let Some(metrics) = crate::ui::canvas::compute_zoom_metrics(
        ctx,
        active_view,
        canvas_size,
        app.settings.fit_all_images_to_window,
        app.settings.pixel_based_1_to_1,
    ) {
        (
            metrics.current_percent,
            metrics.min_percent,
            metrics.max_percent,
            metrics.actual_scale,
            metrics.min_zoom_scale,
        )
    } else {
        (
            active_view.scale * 100.0,
            100.0,
            crate::ui::canvas::MAX_ZOOM_MULTIPLIER * 100.0,
            1.0,
            0.01,
        )
    }
}

fn commit_scale_input(app: &mut ImageApp, ctx: &egui::Context) {
    let input = app.bottom_bar_scale_input.trim();
    if input.is_empty() {
        app.bottom_bar_scale_editing = false;
        return;
    }

    if let Ok(percent) = input.parse::<f32>() {
        let (_, min_percent, max_percent, actual_scale, min_zoom_scale) = scale_bounds_percent(app, ctx);
        let clamped_percent = percent.clamp(min_percent, max_percent);
        let max_zoom_scale = actual_scale * crate::ui::canvas::MAX_ZOOM_MULTIPLIER;
        let new_scale = (actual_scale * (clamped_percent / 100.0))
            .clamp(min_zoom_scale, max_zoom_scale);

        let active_view = app.workspace.active_view_mut();
        active_view.auto_fit = false;
        active_view.scale = new_scale;
        active_view.target_scale = None;
        active_view.target_pan = None;
        active_view.reset_start_time = None;
    }

    app.bottom_bar_scale_editing = false;
    app.bottom_bar_scale_focus_pending = false;
}

fn commit_index_input(app: &mut ImageApp) {
    let input = app.bottom_bar_index_input.trim();
    if input.is_empty() {
        app.bottom_bar_index_editing = false;
        return;
    }

    if let Ok(index_one_based) = input.parse::<usize>() {
        let max_len = app.workspace.active_view().active_playlist.len();
        if max_len > 0 {
            let clamped = index_one_based.clamp(1, max_len);
            crate::handlers::jump_to_index(app, clamped);
        }
    }

    app.bottom_bar_index_editing = false;
    app.bottom_bar_index_focus_pending = false;
}

fn set_cursor_to_end(ctx: &egui::Context, text_id: egui::Id, char_count: usize) {
    if let Some(mut state) = egui::TextEdit::load_state(ctx, text_id) {
        let cursor = egui::text::CCursor::new(char_count);
        let range = egui::text_selection::CCursorRange::one(cursor);
        state.cursor.set_char_range(Some(range));
        state.store(ctx, text_id);
    }
}

fn render_content(app: &mut ImageApp, ctx: &egui::Context, ui: &mut egui::Ui) {
    if app.workspace.content_mode == crate::workspace::ContentMode::PlaylistGrid {
        let playlist = &app.workspace.active_view().active_playlist;
        let total_items = playlist.len();
        let (selected_count, selected_size_bytes, total_size_bytes) = app
            .workspace
            .playlist_grid
            .as_ref()
            .map(|grid| {
                (
                    grid.selection.selected.len(),
                    grid.cached_selected_size_bytes,
                    grid.cached_total_size_bytes,
                )
            })
            .unwrap_or((0, 0, 0));

        let total_label = if total_items == 1 { "item" } else { "items" };
        let right_text = format!(
            "{} {} | {}",
            total_items,
            total_label,
            format_file_size(total_size_bytes)
        );

        let left_text = if selected_count > 0 {
            let selected_label = if selected_count == 1 { "item" } else { "items" };
            Some(format!(
                "{} | {} {} selected",
                format_file_size(selected_size_bytes),
                selected_count,
                selected_label
            ))
        } else {
            None
        };

        ui.horizontal_centered(|ui| {
            ui.add_space(8.0);
            if let Some(text) = left_text {
                let left_label = ui.label(text);
                if left_label.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::Default);
                }
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(8.0);
                let right_label = ui.label(right_text);
                if right_label.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::Default);
                }
            });
        });
        return;
    }

    let active_view_has_target = app.workspace.active_view().current_file_path.is_some() || !app.workspace.active_view().active_playlist.is_empty();
    if !active_view_has_target {
        // Keep geometry stable without consuming the full unconstrained overlay height.
        ui.allocate_space(egui::vec2(ui.available_width(), BOTTOM_BAR_HEIGHT));
        return;
    }

    let has_loaded_image = !app.workspace.active_view().frames.is_empty() && app.workspace.active_view().load_error.is_none();

    // Keep index interaction available even for unsupported/error files.
    // Scale editing depends on an active decoded image, so close it when unavailable.
    if !has_loaded_image {
        app.bottom_bar_scale_editing = false;
        app.bottom_bar_scale_focus_pending = false;
    }

    let size_text = app
        .workspace.active_view()
        .current_file_size_bytes
        .map(format_file_size)
        .unwrap_or_else(|| "--".to_string());

    let resolution_text = app
        .workspace.active_view()
        .image_resolution
        .map(|(w, h)| format!("{}x{}", w, h))
        .unwrap_or_else(|| "--".to_string());

    let (scale_percent, min_percent, max_percent, _actual_scale, _min_zoom_scale) =
        scale_bounds_percent(app, ctx);
    let scale_text = format!("{:.0}%", scale_percent);

    let (index, total) = if app.workspace.active_view().active_playlist.is_empty() {
        (1, 1)
    } else {
        (app.workspace.active_view().current_index + 1, app.workspace.active_view().active_playlist.len())
    };

    let mut scale_input_rect = None;
    let mut index_input_rect = None;

    if !app.bottom_bar_scale_editing && !app.bottom_bar_index_editing {
        app.bottom_bar_edit_just_opened = false;
    }

    // Restored to horizontal_centered to guarantee text correctly anchors to the far left and far right edges.
    ui.horizontal_centered(|ui| {
        ui.add_space(8.0);
        let left_label = ui.label(format!("{}  |  {}", size_text, resolution_text));
        if left_label.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Default);
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_space(8.0);

            ui.horizontal(|ui| {
                let prev_spacing = ui.spacing().item_spacing.x;
                ui.spacing_mut().item_spacing.x = 2.0;

                let suffix = ui.label(format!(" of {}", total));
                if suffix.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::Default);
                }

                if app.bottom_bar_index_editing {
                    app.bottom_bar_index_input.retain(|c| c.is_ascii_digit());
                    let desired_w = 56.0;
                    let text_id = egui::Id::new(INDEX_INPUT_ID);
                    let text_resp = ui.add_sized(
                        [desired_w, ui.spacing().interact_size.y],
                        egui::TextEdit::singleline(&mut app.bottom_bar_index_input)
                            .id_source(text_id)
                            .hint_text(format!("1-{}", total)),
                    );
                    index_input_rect = Some(text_resp.rect);

                    if app.bottom_bar_index_focus_pending {
                        text_resp.request_focus();
                        set_cursor_to_end(ctx, text_id, app.bottom_bar_index_input.chars().count());
                        app.bottom_bar_index_focus_pending = false;
                    }

                    if text_resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        commit_index_input(app);
                    }
                } else {
                    let index_resp = text_like_button(ui, &index.to_string());
                    if index_resp.clicked() {
                        app.bottom_bar_index_editing = true;
                        app.bottom_bar_scale_editing = false;
                        app.bottom_bar_index_input = index.to_string();
                        app.bottom_bar_index_focus_pending = true;
                        app.bottom_bar_scale_focus_pending = false;
                        app.bottom_bar_edit_just_opened = true;
                    }
                }

                ui.spacing_mut().item_spacing.x = prev_spacing;
            });

            let sep = ui.label("|");
            if sep.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Default);
            }

            if has_loaded_image {
                if app.bottom_bar_scale_editing {
                    app.bottom_bar_scale_input.retain(|c| c.is_ascii_digit());
                    let desired_w = 76.0;
                    let text_id = egui::Id::new(SCALE_INPUT_ID);
                    let text_resp = ui.add_sized(
                        [desired_w, ui.spacing().interact_size.y],
                        egui::TextEdit::singleline(&mut app.bottom_bar_scale_input)
                            .id_source(text_id)
                            .hint_text(format!("{:.0}-{:.0}", min_percent, max_percent)),
                    );
                    scale_input_rect = Some(text_resp.rect);

                    if app.bottom_bar_scale_focus_pending {
                        text_resp.request_focus();
                        set_cursor_to_end(ctx, text_id, app.bottom_bar_scale_input.chars().count());
                        app.bottom_bar_scale_focus_pending = false;
                    }

                    if text_resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        commit_scale_input(app, ctx);
                    }
                } else {
                    let scale_resp = text_like_button(ui, &scale_text);
                    if scale_resp.clicked() {
                        app.bottom_bar_scale_editing = true;
                        app.bottom_bar_index_editing = false;
                        app.bottom_bar_scale_input = format!("{:.0}", scale_percent.round());
                        app.bottom_bar_scale_focus_pending = true;
                        app.bottom_bar_index_focus_pending = false;
                        app.bottom_bar_edit_just_opened = true;
                    }
                }
            } else {
                let unavailable_scale = ui.label("--");
                if unavailable_scale.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::Default);
                }
            }
        });
    });

    let is_editing = app.bottom_bar_scale_editing || app.bottom_bar_index_editing;
    if is_editing && !app.bottom_bar_edit_just_opened && ctx.input(|i| i.pointer.any_pressed()) {
        let clicked_outside = ctx.pointer_interact_pos().is_some_and(|pos| {
            let scale_outside = scale_input_rect.is_none_or(|r| !r.contains(pos));
            let index_outside = index_input_rect.is_none_or(|r| !r.contains(pos));
            scale_outside && index_outside
        });

        if clicked_outside {
            if app.bottom_bar_scale_editing {
                commit_scale_input(app, ctx);
            }
            if app.bottom_bar_index_editing {
                commit_index_input(app);
            }
        }
    }

    app.bottom_bar_edit_just_opened = false;
}

pub fn render(app: &mut ImageApp, ctx: &egui::Context) {
    let active_view = app.workspace.active_view();
    let is_single_canvas =
        app.workspace.content_mode == crate::workspace::ContentMode::Canvas && !app.workspace.is_split();
    let is_immersive = is_single_canvas && active_view.is_fullscreen && app.settings.immersive_maximized;
    let is_editing = app.bottom_bar_scale_editing || app.bottom_bar_index_editing;

    if is_immersive {
        let show_bars =
            is_editing
                || app.show_sort_menu
                || app.show_filter_popup
                || app.show_delete_file_dialog
                || app.show_settings_window
                || is_bottom_visible_in_immersive(app, ctx);
        app.immersive_bottombar_visible = show_bars;
        if show_bars {
            egui::Area::new(egui::Id::new("bottom_bar_overlay"))
                .fixed_pos(egui::pos2(0.0, ctx.content_rect().max.y - BOTTOM_BAR_HEIGHT))
                .order(egui::Order::Foreground)
                .show(ctx, |ui| {
                    ui.set_width(ctx.content_rect().width());
                    ui.set_height(BOTTOM_BAR_HEIGHT);

                    // Consume pointer input across the full overlay so canvas interactions
                    // never fire through non-interactive portions of the bar.
                    let _overlay_input = ui.interact(
                        ui.max_rect(),
                        egui::Id::new("bottom_bar_input_shield"),
                        egui::Sense::click_and_drag(),
                    );

                    let active_stroke =
                        egui::Stroke::new(1.0, ui.visuals().strong_text_color().gamma_multiply(0.8));

                    egui::Frame::menu(ui.style()).stroke(active_stroke).show(ui, |ui| {
                        ui.set_height(BOTTOM_BAR_HEIGHT);
                        render_content(app, ctx, ui);
                    });
                });
        }
    } else {
        app.immersive_bottombar_visible = false;

        let mut current_color = if app.is_focused {
            ctx.style().visuals.strong_text_color().gamma_multiply(0.8)
        } else {
            ctx.style().visuals.strong_text_color().gamma_multiply(0.4)
        };

        if app.workspace.active_view().is_fullscreen {
            current_color = current_color.gamma_multiply(0.4);
        }

        let panel_stroke = egui::Stroke::new(1.0, current_color);

        // --- MANUAL VERTICAL CONTROL (WINDOWED MODE) ---
        // By manipulating the inner bounds of the frame, horizontal_centered() will automatically 
        // calculate its center based on the remaining space.
        // Increase `bottom` to push text UP.
        // Increase `top` to push text DOWN.
        let panel_frame = egui::Frame::side_top_panel(&ctx.style())
            .inner_margin(egui::Margin {
                left: 0,
                right: 0,
                top: 0,
                bottom: 3, // <-- Set to 4.0, 5.0, etc., to nudge the entire row up!
            })
            .stroke(egui::Stroke::NONE);

        let panel_res = egui::TopBottomPanel::bottom(egui::Id::new("bottom_status_bar"))
            .frame(panel_frame)
            .show_separator_line(false)
            .exact_height(BOTTOM_BAR_HEIGHT)
            .show(ctx, |ui| {
                // BUG FIX: Removed ui.set_height() here so it doesn't fight horizontal_centered's math
                render_content(app, ctx, ui);
            });

        let rect = panel_res.response.rect;
        ctx.layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            egui::Id::new("bottom_line"),
        ))
        .hline(rect.x_range(), rect.top(), panel_stroke);
    }
}