use crate::app::ImageApp;
use eframe::egui;

const WIDTH_RATIO: f32 = 0.90;
const TOOLBAR_HEIGHT: f32 = 21.0;
const TOOLBAR_BOTTOM_OFFSET: f32 = 68.0;

pub fn render(app: &ImageApp, ctx: &egui::Context, _active_canvas_rect: egui::Rect) {
    if !app.show_floating_toolbar {
        return;
    }

    let content_rect = ctx.content_rect();
    let width = (content_rect.width() * WIDTH_RATIO).min(content_rect.width());
    let height = TOOLBAR_HEIGHT;
    let y_min = content_rect.max.y - TOOLBAR_BOTTOM_OFFSET - height;

    let x_min = content_rect.center().x - width * 0.5;

    egui::Area::new(egui::Id::new("floating_toolbar_overlay"))
        .fixed_pos(egui::pos2(x_min, y_min))
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            ui.set_width(width);
            ui.set_height(height);

            let _overlay_input = ui
                .interact(
                    ui.max_rect(),
                    egui::Id::new("floating_toolbar_input_shield"),
                    egui::Sense::click_and_drag(),
                )
                .on_hover_cursor(egui::CursorIcon::Default)
                .on_hover_and_drag_cursor(egui::CursorIcon::Default);

            let active_stroke =
                egui::Stroke::new(1.0, ui.visuals().strong_text_color().gamma_multiply(0.8));

            egui::Frame::menu(ui.style())
                .stroke(active_stroke)
                .show(ui, |ui| {
                    ui.set_width(width);
                    ui.set_height(height);
                    ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::hover());
                });
        });
}
