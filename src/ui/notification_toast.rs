use eframe::egui;

use crate::app::ImageApp;

const PADDING_X: f32 = 16.0;
const PADDING_Y: f32 = 12.0;
pub fn render(app: &mut ImageApp, ctx: &egui::Context) {
    let now = ctx.input(|i| i.time);
    let Some(message) = app.notifications.message(now) else {
        return;
    };

    let bottom_offset = PADDING_Y + crate::ui::bottom_bar::IMMERSIVE_BOTTOM_BAR_OVERLAY_HEIGHT;

    egui::Area::new(egui::Id::new("notification_toast"))
        .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-PADDING_X, -bottom_offset))
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            egui::Frame::menu(ui.style()).show(ui, |ui| {
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.add_space(8.0);
                    ui.label(message);
                    ui.add_space(8.0);
                });
                ui.add_space(6.0);
            });
        });

    ctx.request_repaint();
}
