use crate::app::ImageApp;
use eframe::egui;

mod items;

const WIDTH_RATIO: f32 = 0.90;
const TOOLBAR_HEIGHT: f32 = 21.0;
const TOOLBAR_BOTTOM_OFFSET: f32 = 68.0;
const ITEM_SPACING: f32 = 6.0;

pub fn render(app: &mut ImageApp, ctx: &egui::Context, _active_canvas_rect: egui::Rect) {
    if !app.show_floating_toolbar {
        return;
    }

    let width = ctx.content_rect().width() * WIDTH_RATIO;
    let height = TOOLBAR_HEIGHT;

    egui::Area::new(egui::Id::new("floating_toolbar_overlay"))
        .anchor(
            egui::Align2::CENTER_BOTTOM,
            egui::vec2(0.0, -TOOLBAR_BOTTOM_OFFSET),
        )
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            ui.set_min_size(egui::vec2(width, height));

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
                    ui.set_min_size(egui::vec2(width, height));
                    let previous_spacing = ui.spacing().item_spacing;
                    ui.spacing_mut().item_spacing.x = ITEM_SPACING;
                    let layout =
                        egui::Layout::left_to_right(egui::Align::Center).with_main_align(egui::Align::Center);
                    ui.allocate_ui_with_layout(ui.available_size(), layout, |ui| {
                        items::render_split_pan_zoom_sync_toggle(app, ui);
                    });
                    ui.spacing_mut().item_spacing = previous_spacing;
                });
        });
}
