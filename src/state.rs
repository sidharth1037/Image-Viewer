use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};
use eframe::egui::TextureHandle;
use crate::image_io::LoadedImage;

pub struct ViewerState {
    pub is_fullscreen: bool,
    pub current_file_name: String,
    
    // --- Image Data ---
    pub texture: Option<TextureHandle>,
    
    // --- Communication Channels ---
    pub req_tx: Sender<PathBuf>,
    pub res_rx: Receiver<Result<LoadedImage, String>>,
}

impl ViewerState {
    // We now require the channels to be passed in when creating the state
    pub fn new(req_tx: Sender<PathBuf>, res_rx: Receiver<Result<LoadedImage, String>>) -> Self {
        Self {
            is_fullscreen: false,
            current_file_name: String::new(),
            texture: None,
            req_tx,
            res_rx,
        }
    }
}