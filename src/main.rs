// This attribute tells Windows not to open a console window when the app starts.
// #![windows_subsystem = "windows"]

// Declare our modules (links the .rs files)
mod state;
mod app;
mod image_io;
mod scanner;
#[cfg(windows)]
mod win32;

use app::ImageApp;

fn main() -> eframe::Result<()> {

    // Collect arguments. args[0] is the exe path, args[1] is the file path.
    let args: Vec<String> = std::env::args().collect();
    let initial_file = args.get(1).cloned();

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
        Box::new(|cc| Ok(Box::new(ImageApp::new(cc, initial_file)))),
    )
}