use eframe::egui;

use crate::app::ImageApp;

const OFFSET_X: f32 = 14.0;
const OFFSET_Y: f32 = 16.0;

pub fn render(app: &ImageApp, ctx: &egui::Context) {
    let Some(payload) = app.group_drag_payload.as_ref() else {
        return;
    };

    let Some(pointer_pos) = ctx.pointer_interact_pos() else {
        return;
    };

    let label = build_label(payload);
    let pos = pointer_pos + egui::vec2(OFFSET_X, OFFSET_Y);

    egui::Area::new(egui::Id::new("group_drag_preview"))
        .fixed_pos(pos)
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            egui::Frame::menu(ui.style()).show(ui, |ui| {
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.add_space(6.0);
                    ui.label(label);
                    ui.add_space(6.0);
                });
                ui.add_space(4.0);
            });
        });
}

fn build_label(payload: &crate::groups::GroupDragPayload) -> String {
    let count = payload.paths.len();
    if count == 1 {
        payload.paths[0]
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "1 item".to_string())
    } else {
        format!("{} items", count)
    }
}
