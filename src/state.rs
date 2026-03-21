use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};
use eframe::egui::{TextureHandle, Vec2}; 

pub struct ViewerState {
    pub is_fullscreen: bool,
    pub current_file_name: String,
    
    // --- Camera Math ---
    pub auto_fit: bool,   // Toggles between "fit to window" and "free zoom/pan"
    pub scale: f32,       // 1.0 = 100%, 2.0 = 200%, etc.
    pub pan: Vec2,        // The X/Y offset of the image from the center
    
    // --- Animation Targets ---
    pub target_scale: Option<f32>, // The scale we are moving toward
    pub target_pan: Option<Vec2>,   // The pan offset we are moving toward
    pub reset_start_time: Option<f64>, // Stores the timestamp when double-click happened

// --- Image Data & Animation ---
    pub frames: Vec<TextureHandle>,
    pub frame_durations: Vec<f64>, // Stored in seconds for easy math
    pub current_frame: usize,
    pub last_frame_time: Option<f64>,
    pub load_error: Option<String>,
    
// --- Communication Channels ---
    pub req_tx: Sender<PathBuf>,
    pub res_rx: Receiver<Result<crate::image_io::LoadedImage, (String, String)>>,

    // --- Playlist State ---
    pub current_folder: Option<PathBuf>,
    pub playlist: Vec<PathBuf>,
    pub current_index: usize,
    pub sort_method: crate::scanner::SortMethod, // Store the active setting
    pub dir_req_tx: Sender<crate::scanner::ScanRequest>, // Updated type
    pub dir_res_rx: Receiver<crate::scanner::DirectoryState>,
}

impl ViewerState {
    pub fn new(
        req_tx: Sender<PathBuf>,
        res_rx: Receiver<Result<crate::image_io::LoadedImage, (String, String)>>,
        dir_req_tx: Sender<crate::scanner::ScanRequest>,
        dir_res_rx: Receiver<crate::scanner::DirectoryState>
    ) -> Self {

        Self {
            is_fullscreen: false,
            current_file_name: String::new(),

            auto_fit: true,       
            scale: 1.0,           
            pan: Vec2::ZERO,
 
            target_scale: None,
            target_pan: None,
            reset_start_time: None,

            frames: Vec::new(),
            frame_durations: Vec::new(),
            current_frame: 0,
            last_frame_time: None,
            load_error: None,

            req_tx,
            res_rx,

            current_folder: None,
            playlist: Vec::new(),
            current_index: 0,
            sort_method: crate::scanner::SortMethod::Lexical, // Default sorting
            dir_req_tx,
            dir_res_rx,
        }
    }
}