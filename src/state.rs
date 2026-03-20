use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};
use eframe::egui::{TextureHandle, Vec2}; 
use crate::image_io::LoadedImage;

pub struct ViewerState {
    pub is_fullscreen: bool,
    pub current_file_name: String,
    
    // --- Camera Math ---
    pub auto_fit: bool,   // Toggles between "fit to window" and "free zoom/pan"
    pub scale: f32,       // 1.0 = 100%, 2.0 = 200%, etc.
    pub pan: Vec2,        // The X/Y offset of the image from the center

    // --- Image Data ---
    pub texture: Option<TextureHandle>,
    
    // --- Communication Channels ---
    pub req_tx: Sender<PathBuf>,
    pub res_rx: Receiver<Result<LoadedImage, String>>,
}

impl ViewerState {
    pub fn new(req_tx: Sender<PathBuf>, res_rx: Receiver<Result<LoadedImage, String>>) -> Self {
        Self {
            is_fullscreen: false,
            current_file_name: String::new(),
            auto_fit: true,       // Start by automatically fitting the image to the window
            scale: 1.0,           
            pan: Vec2::ZERO,      
            texture: None,
            req_tx,
            res_rx,
        }
    }
}