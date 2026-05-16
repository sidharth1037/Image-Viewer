use eframe::egui;
use egui_phosphor::regular as icons;
use crate::app::ImageApp;

pub fn render(app: &mut ImageApp, ctx: &egui::Context) {
    if !app.show_settings_window {
        return;
    }

    egui::Window::new(format!("{} Settings", icons::GEAR_SIX))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            
            // --- WINDOW BEHAVIOR ---
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label(icons::MONITOR_PLAY); 
                ui.heading("Window Behavior");
            });
            ui.add_space(4.0);
            
            ui.checkbox(&mut app.settings.immersive_maximized, "Immersive Maximized Mode")
                .on_hover_text("When maximized, auto-hide the top bar to show the image in full screen.");

            ui.checkbox(&mut app.settings.fit_all_images_to_window, "Fit all images to window")
                .on_hover_text("When enabled, newly shown images start fitted to the canvas. Small images can still zoom out to actual size.");

            ui.checkbox(&mut app.settings.pixel_based_1_to_1, "Use pixel-based 1:1 scale")
                .on_hover_text("When enabled, 100% scale maps one image pixel to one screen pixel. If disabled, true-size mode can use DPI metadata when available.");
            
            ui.add_space(12.0);

            // --- FOLDER VIEW ---
            ui.horizontal(|ui| {
                ui.label(icons::FILE_IMAGE);
                ui.heading("Folder View");
            });
            ui.add_space(4.0);

            let thumb_steps = [60u32, 80, 100, 120, 140, 160, 180, 200, 220];
            let mut step_index = thumb_steps
                .iter()
                .position(|&value| value == app.settings.thumbnail_width)
                .unwrap_or(5);
            let mut current_width = thumb_steps[step_index];

            ui.horizontal(|ui| {
                ui.label("Thumbnail size");
                let slider = egui::Slider::new(&mut step_index, 0..=8).show_value(false);
                if ui.add(slider).changed() {
                    current_width = thumb_steps[step_index];
                    app.settings.thumbnail_width = current_width;
                    if let Some(grid) = app.workspace.playlist_grid.as_mut() {
                        grid.settings.thumbnail_width = current_width;
                        grid.thumbnail_cache.clear();
                        grid.pending_requests.clear();
                    }
                }
                ui.label(format!("{} px", current_width));
            });

            // --- NAVIGATION ---
            ui.horizontal(|ui| {
                ui.label(icons::ARROWS_LEFT_RIGHT); 
                ui.heading("Navigation");
            });
            ui.add_space(4.0);

            ui.checkbox(&mut app.settings.loop_playlist, "Loop Playlist")
                .on_hover_text("Wrap around to the beginning or end when navigating past the last or first image in a folder.");

            ui.add_space(16.0);
            ui.separator();
            ui.add_space(8.0);
            
            // --- BOTTOM BAR ---
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(format!("{} OK", icons::CHECK)).clicked() {
                        app.show_settings_window = false;
                    }
                });
            });
        });
}