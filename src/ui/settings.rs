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
            
            ui.add_space(12.0);

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