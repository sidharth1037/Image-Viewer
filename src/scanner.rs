use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};

// --- Extensible Sorting Configuration ---
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SortMethod {
    Alphabetical, 
    Natural,      
    Size,         
    DateModified, 
    DateCreated,  // <-- NEW
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

pub fn spawn_directory_scanner() -> (Sender<ScanRequest>, Receiver<DirectoryState>) {
    let (req_tx, req_rx) = channel::<ScanRequest>();
    let (res_tx, res_rx) = channel::<DirectoryState>();

    std::thread::spawn(move || {
        let valid_exts = ["webp", "avif", "jxl", "png", "jpg", "jpeg", "gif", "tif", "tiff", "bmp", "ico"];

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

                // --- Strategy Routing based on Enum ---
                match request.sort_method {
                    SortMethod::Alphabetical => {
                        playlist.sort_by(|a, b| {
                            let name_a = a.file_name().unwrap_or_default().to_string_lossy().to_lowercase();
                            let name_b = b.file_name().unwrap_or_default().to_string_lossy().to_lowercase();
                            name_a.cmp(&name_b)
                        });
                    }
                    SortMethod::Natural => {
                        playlist.sort_by(|a, b| {
                            let name_a = a.file_name().unwrap_or_default().to_string_lossy();
                            let name_b = b.file_name().unwrap_or_default().to_string_lossy();
                            lexical_sort::natural_lexical_cmp(&name_a, &name_b)
                        });
                    }
                    SortMethod::Size => {
                        // sort_by_cached_key prevents excessive disk I/O
                        playlist.sort_by_cached_key(|k| {
                            std::fs::metadata(k).map(|m| m.len()).unwrap_or(0)
                        });
                    }
                    SortMethod::DateModified => {
                        playlist.sort_by_cached_key(|k| {
                            std::fs::metadata(k)
                                .and_then(|m| m.modified())
                                .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
                        });
                    }
                    SortMethod::DateCreated => {
                        playlist.sort_by_cached_key(|k| {
                            std::fs::metadata(k)
                                // Some OS/Filesystems don't support created(), fallback to modified
                                .and_then(|m| m.created().or_else(|_| m.modified()))
                                .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
                        });
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