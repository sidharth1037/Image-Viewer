use eframe::egui;

use crate::app::ImageApp;
use crate::groups::{DEFAULT_GROUP_ID, DEFAULT_GROUP_NAME};
use crate::handlers;

pub fn render(app: &mut ImageApp, ctx: &egui::Context) {
    if !app.show_group_assign_prompt {
        return;
    }

    let mut selected_group: Option<u32> = None;
    let mut create_group = false;

    egui::Window::new("Add to group")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.set_min_width(220.0);
            ui.vertical(|ui| {
                if ui.button(DEFAULT_GROUP_NAME).clicked() {
                    selected_group = Some(DEFAULT_GROUP_ID);
                }

                for group in app.workspace.group_tabs.user_groups.iter() {
                    if ui.button(&group.name).clicked() {
                        selected_group = Some(group.id);
                    }
                }

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(6.0);

                if ui.button("+ Create new group").clicked() {
                    create_group = true;
                }
            });
        });

    if create_group {
        let new_id = app.workspace.group_tabs.add_group();
        selected_group = Some(new_id);
    }

    if let Some(group_id) = selected_group {
        let time = ctx.input(|i| i.time);
        handlers::apply_group_assign_prompt(app, group_id, time);
    }
}
