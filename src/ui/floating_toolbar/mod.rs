use crate::app::ImageApp;
use eframe::egui;

mod items;

const TOOLBAR_BOTTOM_OFFSET: f32 = 68.0;
const ITEM_SPACING: f32 = 6.0;
const TOOLBAR_PADDING: i8 = 4;

pub fn render(app: &mut ImageApp, ctx: &egui::Context, _active_canvas_rect: egui::Rect) {
    if !app.show_floating_toolbar {
        return;
    }

    let active_stroke =
        egui::Stroke::new(1.0, ctx.style().visuals.strong_text_color().gamma_multiply(0.8));

    egui::Area::new(egui::Id::new("floating_toolbar_overlay"))
        .anchor(
            egui::Align2::CENTER_BOTTOM,
            egui::vec2(0.0, -TOOLBAR_BOTTOM_OFFSET),
        )
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            // Input shield: absorb clicks/drags so they don't fall through to the canvas.
            let _shield = ui
                .interact(
                    ui.max_rect(),
                    egui::Id::new("floating_toolbar_input_shield"),
                    egui::Sense::click_and_drag(),
                )
                .on_hover_cursor(egui::CursorIcon::Default)
                .on_hover_and_drag_cursor(egui::CursorIcon::Default);

            egui::Frame::menu(ui.style())
                .stroke(active_stroke)
                .inner_margin(egui::Margin::same(TOOLBAR_PADDING))
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.x = ITEM_SPACING;

                    ui.horizontal_centered(|ui| {
                        // --- toolbar items ---
                        items::render_carry_adjustments_toggle(app, ui);
                        items::render_split_pan_zoom_sync_toggle(app, ui);
                        // Future items go here.
                    });
                });
        });
}
