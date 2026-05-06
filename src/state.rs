use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use eframe::egui::{TextureHandle, Vec2}; 
use crate::preload::PreloadRing;
use crate::adjustments::AdjustmentPipeline;

#[derive(Clone, Default)]
pub struct FilterCriteria {
    pub text: String,
}

#[derive(Clone, Default)]
pub struct FilterState {
    pub criteria: FilterCriteria,
}

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
    pub last_canvas_size: Vec2,

// --- Image Data & Animation ---
    pub frames: Vec<TextureHandle>,
    pub frame_durations: Vec<f64>, 
    pub current_frame: usize,
    pub last_frame_time: Option<f64>,
    pub image_resolution: Option<(u32, u32)>,
    pub image_density: Option<crate::image_io::ImageDensity>,
    pub current_file_size_bytes: Option<u64>,
    pub load_error: Option<String>,
    
    // --- Async Communication ---
    pub load_id: Arc<AtomicU64>, // Cancellation token
    pub req_tx: Sender<(PathBuf, u64)>, // Sends (Path, VersionID)
    pub res_rx: Receiver<Result<crate::image_io::LoadedImage, crate::image_io::LoadFailure>>,

    // --- Playlist State ---
    pub current_folder: Option<PathBuf>,
    pub source_playlist: Vec<PathBuf>,
    pub active_playlist: Vec<PathBuf>,
    pub current_index: usize,
    pub filter: FilterState,
    pub sort_method: crate::scanner::SortMethod, 
    pub sort_order: crate::scanner::SortOrder,
    pub scan_id: Arc<AtomicU64>, // Cancellation token for folder scans
    pub dir_req_tx: Sender<crate::scanner::ScanRequest>, 
    pub dir_res_rx: Receiver<crate::scanner::DirectoryState>,

    // --- Preloading Ring Buffer ---
    pub preload: PreloadRing,

    // --- Image Adjustments (Gamma, future: Contrast, Exposure, etc.) ---
    pub adjustments: AdjustmentPipeline,
    pub original_pixels: Vec<Vec<u8>>,  // Original decoded pixels per frame (untouched by adjustments)
    pub adjustments_dirty: bool,        // True when textures need rebuilding after an adjustment change
    pub rotation_quarter_turns: u8,
    pub overlay_last_changed: Option<f64>,
    pub overlay_text: Option<String>,
    pub show_original_while_held: bool,
    pub carry_adjustments: bool,
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
            last_canvas_size: Vec2::ZERO,
            frames: Vec::new(),
            frame_durations: Vec::new(),
            current_frame: 0,
            last_frame_time: None,
            image_resolution: None,
            image_density: None,
            current_file_size_bytes: None,
            load_error: None,
            load_id,
            req_tx,
            res_rx,
            current_folder: None,
            source_playlist: Vec::new(),
            active_playlist: Vec::new(),
            current_index: 0,
            filter: FilterState::default(),
            sort_method: crate::scanner::SortMethod::Natural,
            sort_order: crate::scanner::default_order_for(crate::scanner::SortMethod::Natural),
            scan_id,
            dir_req_tx,
            dir_res_rx,
            preload,
            adjustments: AdjustmentPipeline::default(),
            original_pixels: Vec::new(),
            adjustments_dirty: false,
            rotation_quarter_turns: 0,
            overlay_last_changed: None,
            overlay_text: None,
            show_original_while_held: false,
            carry_adjustments: false,
        }
    }

    pub fn clone_for_compare(&self, ctx: &eframe::egui::Context) -> Self {
        let load_id = Arc::new(AtomicU64::new(0));
        let preload_epoch = Arc::new(AtomicU64::new(0));
        let scan_id = Arc::new(AtomicU64::new(0));
        let (req_tx, res_rx) = crate::image_io::spawn_image_loader(ctx.clone(), load_id.clone());
        let (preload_req_tx, preload_res_rx) = crate::image_io::spawn_image_loader_ordered(ctx.clone(), preload_epoch.clone());
        let (dir_req_tx, dir_res_rx) = crate::scanner::spawn_directory_scanner(scan_id.clone()); 
        let preload = PreloadRing::new(preload_epoch, preload_req_tx, preload_res_rx);

        let mut next_state = Self::new(load_id.clone(), req_tx, res_rx, scan_id, dir_req_tx, dir_res_rx, preload);
        next_state.current_folder = self.current_folder.clone();
        next_state.source_playlist = self.source_playlist.clone();
        next_state.active_playlist = self.active_playlist.clone();
        next_state.current_index = self.current_index;
        next_state.filter = FilterState { criteria: self.filter.criteria.clone() };
        next_state.sort_method = self.sort_method.clone();
        next_state.sort_order = self.sort_order.clone();

        // Load the image naturally by queuing a request
        if let Some(path) = &self.current_file_path {
            let version = load_id.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
            let _ = next_state.req_tx.send((path.clone(), version));
        }
        next_state
    }
}