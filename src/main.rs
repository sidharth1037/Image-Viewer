// Declare our modules (links the .rs files)
mod state;
mod app;
#[cfg(windows)]
mod win32;

use app::ImageApp;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([640.0, 480.0])
            .with_min_inner_size([300.0, 300.0]) // Set minimum resize limit
            .with_decorations(false),
        ..Default::default() // Use default values for the rest of the config
    };

    eframe::run_native(
        "Image Viewer", // Title in the OS taskbar
        options,
        // This is a closure (lambda). It creates the app instance.
        // Box::new puts our app on the Heap memory.
        Box::new(|cc| Ok(Box::new(ImageApp::new(cc)))),
    )
}