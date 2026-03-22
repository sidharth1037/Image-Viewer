use crate::app::ImageApp;
use eframe::egui;

const BOTTOM_BAR_HEIGHT: f32 = 28.0;
const EDGE_TRIGGER_HEIGHT: f32 = 34.0;

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

fn render_content(app: &ImageApp, ui: &mut egui::Ui) {
    let has_loaded_image = !app.state.frames.is_empty() && app.state.load_error.is_none();
    if !has_loaded_image {
        // Keep the bar geometry stable even with no visible text.
        ui.allocate_space(egui::vec2(ui.available_width(), ui.available_height().max(1.0)));
        return;
    }

    let size_text = app
        .state
        .current_file_size_bytes
        .map(format_file_size)
        .unwrap_or_else(|| "--".to_string());

    let resolution_text = app
        .state
        .image_resolution
        .map(|(w, h)| format!("{}x{}", w, h))
        .unwrap_or_else(|| "--".to_string());

    let scale_text = format!("{:.0}%", app.state.scale * 100.0);

    let (index, total) = if app.state.playlist.is_empty() {
        (1, 1)
    } else {
        (app.state.current_index + 1, app.state.playlist.len())
    };
    let playlist_text = format!("{} of {}", index, total);

    // Restored to horizontal_centered to guarantee text correctly anchors to the far left and far right edges.
    ui.horizontal_centered(|ui| {
        ui.add_space(8.0);
        ui.label(format!("{}  |  {}", size_text, resolution_text));

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_space(8.0);
            ui.label(playlist_text);
            ui.label("|");
            ui.label(scale_text);
        });
    });
}

pub fn render(app: &ImageApp, ctx: &egui::Context) {
    let is_immersive = app.state.is_fullscreen && app.settings.immersive_maximized;

    if is_immersive {
        let show_bars = app.show_sort_menu || is_bottom_visible_in_immersive(app, ctx);
        if show_bars {
            egui::Area::new(egui::Id::new("bottom_bar_overlay"))
                .fixed_pos(egui::pos2(0.0, ctx.content_rect().max.y - BOTTOM_BAR_HEIGHT))
                .order(egui::Order::Foreground)
                .show(ctx, |ui| {
                    ui.set_width(ctx.content_rect().width());
                    let active_stroke =
                        egui::Stroke::new(1.0, ui.visuals().strong_text_color().gamma_multiply(0.8));

                    egui::Frame::menu(ui.style()).stroke(active_stroke).show(ui, |ui| {
                        ui.set_min_height(BOTTOM_BAR_HEIGHT);
                        render_content(app, ui);
                    });
                });
        }
    } else {
        let mut current_color = if app.is_focused {
            ctx.style().visuals.strong_text_color().gamma_multiply(0.8)
        } else {
            ctx.style().visuals.strong_text_color().gamma_multiply(0.4)
        };

        if app.state.is_fullscreen {
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
                render_content(app, ui);
            });

        let rect = panel_res.response.rect;
        ctx.layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            egui::Id::new("bottom_line"),
        ))
        .hline(rect.x_range(), rect.top(), panel_stroke);
    }
}