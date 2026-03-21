use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};

// --- NEW: Extensible Sorting Configuration ---
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SortMethod {
    Lexical,
    Size,         // Placeholder for future
    DateModified, // Placeholder for future
}

pub struct ScanRequest {
    pub target_path: PathBuf,
    pub sort_method: SortMethod,
}

pub struct DirectoryState {
    pub folder_path: PathBuf,
    pub playlist: Vec<PathBuf>,
    pub current_index: usize,
}

// Note the channel now takes ScanRequest instead of PathBuf
pub fn spawn_directory_scanner() -> (Sender<ScanRequest>, Receiver<DirectoryState>) {
    let (req_tx, req_rx) = channel::<ScanRequest>();
    let (res_tx, res_rx) = channel::<DirectoryState>();

    std::thread::spawn(move || {
        let valid_exts = ["webp", "avif", "jxl", "png", "jpg", "jpeg", "gif", "tif", "tiff", "bmp", "ico"];

        // Listen for the new structured request
        while let Ok(request) = req_rx.recv() {
            if let Some(folder) = request.target_path.parent() {
                let mut playlist = Vec::new();
                
                if let Ok(entries) = std::fs::read_dir(folder) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_file() {
                            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                                if valid_exts.contains(&ext.to_lowercase().as_str()) {
                                    playlist.push(path);
                                }
                            }
                        }
                    }
                }

                // --- NEW: Strategy Routing based on Enum ---
                match request.sort_method {
                    SortMethod::Lexical => {
                        playlist.sort_by(|a, b| {
                            let name_a = a.file_name().unwrap_or_default().to_string_lossy();
                            let name_b = b.file_name().unwrap_or_default().to_string_lossy();
                            lexical_sort::natural_lexical_cmp(&name_a, &name_b)
                        });
                    }
                    SortMethod::Size => {
                        // TODO: Implement metadata size sorting
                        println!("Size sorting not yet implemented, falling back to Lexical.");
                    }
                    SortMethod::DateModified => {
                        // TODO: Implement metadata date sorting
                        println!("Date sorting not yet implemented, falling back to Lexical.");
                    }
                }

                let current_index = playlist.iter().position(|p| p == &request.target_path).unwrap_or(0);

                let _ = res_tx.send(DirectoryState {
                    folder_path: folder.to_path_buf(),
                    playlist,
                    current_index,
                });
            }
        }
    });

    (req_tx, res_rx)
}