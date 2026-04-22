// This attribute tells Windows not to open a console window when the app starts.
#![windows_subsystem = "windows"]

// Declare our modules (links the .rs files)
mod state;
mod app;
mod adjustments;
mod image_io;
mod preload;
mod scanner;
mod playlist_view;
mod shortcuts;
mod persistence;
mod ui;
mod handlers;
mod workspace;
mod file_ops;
#[cfg(windows)]
mod win32;
#[cfg(windows)]
mod platform;

use app::ImageApp;

fn main() -> eframe::Result<()> {

    // Collect arguments. args[0] is the exe path, args[1] is the file path.
    let args: Vec<String> = std::env::args().collect();
    let initial_file = args.get(1).cloned();

    let viewport = eframe::egui::ViewportBuilder::default()
        .with_inner_size([640.0, 480.0])
        .with_min_inner_size([450.0, 450.0])
        .with_decorations(false);

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default() // Use default values for the rest of the config
    };

    eframe::run_native(
        "Image Viewer", // Title in the OS taskbar
        options,
        // This is a closure (lambda). It creates the app instance.
        // Box::new puts our app on the Heap memory.
        Box::new(|cc| {
            let persisted_state = persistence::load_persisted_state();

            cc.egui_ctx.set_visuals(eframe::egui::Visuals::dark());

            let mut fonts = eframe::egui::FontDefinitions::default();
            egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
            cc.egui_ctx.set_fonts(fonts);

            Ok(Box::new(ImageApp::new(cc, initial_file, persisted_state)))
        }),
    )
}