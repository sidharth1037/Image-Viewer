use eframe::egui;

use crate::app::ImageApp;
use crate::handlers;

pub fn render(app: &mut ImageApp, ctx: &egui::Context) {
    if !app.show_group_assign_prompt {
        return;
    }

    // Close on Escape key.
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        app.show_group_assign_prompt = false;
        app.group_assign_prompt_path = None;
        return;
    }

    let mut selected_group: Option<u32> = None;
    let mut create_group = false;
    let mut cancel = false;

    egui::Window::new("Add to group")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.set_min_width(220.0);
            ui.vertical(|ui| {
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

                ui.add_space(4.0);

                if ui.button("Cancel").clicked() {
                    cancel = true;
                }
            });
        });

    if cancel {
        app.show_group_assign_prompt = false;
        app.group_assign_prompt_path = None;
        return;
    }

    if create_group {
        let new_id = app.workspace.group_tabs.add_group();
        selected_group = Some(new_id);
    }

    if let Some(group_id) = selected_group {
        let time = ctx.input(|i| i.time);
        handlers::apply_group_assign_prompt(app, group_id, time);
    }
}
