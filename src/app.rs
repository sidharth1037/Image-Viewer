use eframe::egui;
use crate::state::ViewerState;

pub struct ImageApp {
    state: ViewerState,
}

impl ImageApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self {
            state: ViewerState::new(),
        }
    }
}

impl eframe::App for ImageApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |_ui| {
            // Blank screen, no UI elements for now
        });
    }
}