use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use eframe::egui::{TextureHandle, Vec2}; 
use crate::preload::PreloadRing;
use crate::adjustments::AdjustmentPipeline;

pub struct ViewerState {
    // NOTE: This tracks viewport maximized state (not OS exclusive fullscreen mode).
    pub is_fullscreen: bool,
    pub current_file_path: Option<PathBuf>,
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
    pub frame_durations: Vec<f64>, 
    pub current_frame: usize,
    pub last_frame_time: Option<f64>,
    pub image_resolution: Option<(u32, u32)>,
    pub current_file_size_bytes: Option<u64>,
    pub load_error: Option<String>,
    
    // --- Async Communication ---
    pub load_id: Arc<AtomicU64>, // Cancellation token
    pub req_tx: Sender<(PathBuf, u64)>, // Sends (Path, VersionID)
    pub res_rx: Receiver<Result<crate::image_io::LoadedImage, crate::image_io::LoadFailure>>,

    // --- Playlist State ---
    pub current_folder: Option<PathBuf>,
    pub playlist: Vec<PathBuf>,
    pub current_index: usize,
    pub sort_method: crate::scanner::SortMethod, 
    pub scan_id: Arc<AtomicU64>, // Cancellation token for folder scans
    pub dir_req_tx: Sender<crate::scanner::ScanRequest>, 
    pub dir_res_rx: Receiver<crate::scanner::DirectoryState>,

    // --- Preloading Ring Buffer ---
    pub preload: PreloadRing,

    // --- Image Adjustments (Gamma, future: Contrast, Exposure, etc.) ---
    pub adjustments: AdjustmentPipeline,
    pub original_pixels: Vec<Vec<u8>>,  // Original decoded pixels per frame (untouched by adjustments)
    pub adjustments_dirty: bool,        // True when textures need rebuilding after an adjustment change
    pub adjustments_last_changed: Option<f64>, // Timestamp of last adjustment change (for fade-out overlay)
}

impl ViewerState {
    pub fn new(
        load_id: Arc<AtomicU64>,
        req_tx: Sender<(PathBuf, u64)>,
        res_rx: Receiver<Result<crate::image_io::LoadedImage, crate::image_io::LoadFailure>>,
        scan_id: Arc<AtomicU64>,
        dir_req_tx: Sender<crate::scanner::ScanRequest>,
        dir_res_rx: Receiver<crate::scanner::DirectoryState>,
        preload: PreloadRing,
    ) -> Self {
        Self {
            is_fullscreen: false,
            current_file_path: None,
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
            image_resolution: None,
            current_file_size_bytes: None,
            load_error: None,
            load_id,
            req_tx,
            res_rx,
            current_folder: None,
            playlist: Vec::new(),
            current_index: 0,
            sort_method: crate::scanner::SortMethod::Natural,
            scan_id,
            dir_req_tx,
            dir_res_rx,
            preload,
            adjustments: AdjustmentPipeline::default(),
            original_pixels: Vec::new(),
            adjustments_dirty: false,
            adjustments_last_changed: None,
        }
    }
}