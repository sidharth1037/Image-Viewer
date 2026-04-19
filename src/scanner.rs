use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use serde::{Deserialize, Serialize};

// --- Extensible Sorting Configuration ---
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SortMethod {
    Alphabetical, 
    Natural,      
    Size,         
    DateModified, 
    DateCreated,  // <-- NEW
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SortOrder {
    Ascending,
    Descending,
}

impl SortOrder {
    pub fn toggled(self) -> Self {
        match self {
            Self::Ascending => Self::Descending,
            Self::Descending => Self::Ascending,
        }
    }
}

pub fn default_order_for(method: SortMethod) -> SortOrder {
    match method {
        SortMethod::Size => SortOrder::Descending,
        SortMethod::Alphabetical
        | SortMethod::Natural
        | SortMethod::DateModified
        | SortMethod::DateCreated => SortOrder::Ascending,
    }
}

pub struct ScanRequest {
    pub target_path: PathBuf,
    pub sort_method: SortMethod,
    pub sort_order: SortOrder,
    pub request_id: u64,
}

pub struct DirectoryState {
    pub request_id: u64,
    pub folder_path: PathBuf,
    pub playlist: Vec<PathBuf>,
}

pub fn spawn_directory_scanner(id_tracker: Arc<AtomicU64>) -> (Sender<ScanRequest>, Receiver<DirectoryState>) {
    let (req_tx, req_rx) = channel::<ScanRequest>();
    let (res_tx, res_rx) = channel::<DirectoryState>();

    std::thread::spawn(move || {
        let valid_exts = [
            "webp", "avif", "heic", "heif", "hif", "jxl", "png", "jpg", "jpeg", "gif", "tif",
            "tiff", "bmp", "ico",
        ];

        while let Ok(mut request) = req_rx.recv() {
            // Keep only the latest scan request to avoid stale scans after rapid UI actions.
            while let Ok(newer_request) = req_rx.try_recv() {
                request = newer_request;
            }

            if id_tracker.load(Ordering::Acquire) != request.request_id {
                continue;
            }

            if let Some(folder) = request.target_path.parent() {
                let mut playlist = Vec::new();
                
                if let Ok(entries) = std::fs::read_dir(folder) {
                    for entry in entries.flatten() {
                        if id_tracker.load(Ordering::Acquire) != request.request_id {
                            playlist.clear();
                            break;
                        }

                        let path = entry.path();
                        if path.is_file() {
                            // Always include the currently targeted file, even if it has no/odd extension.
                            let include_target = path == request.target_path;
                            let include_by_ext = path
                                .extension()
                                .and_then(|e| e.to_str())
                                .map(|ext| valid_exts.contains(&ext.to_lowercase().as_str()))
                                .unwrap_or(false);

                            if include_target || include_by_ext {
                                playlist.push(path);
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
                        let mut with_meta: Vec<(PathBuf, Option<std::fs::Metadata>)> =
                            playlist.into_iter().map(|p| {
                                let meta = std::fs::metadata(&p).ok();
                                (p, meta)
                            }).collect();

                        with_meta.sort_by_cached_key(|(_, meta)| {
                            meta.as_ref().map(|m| m.len()).unwrap_or(0)
                        });

                        playlist = with_meta.into_iter().map(|(p, _)| p).collect();
                    }
                    SortMethod::DateModified => {
                        let mut with_meta: Vec<(PathBuf, Option<std::fs::Metadata>)> =
                            playlist.into_iter().map(|p| {
                                let meta = std::fs::metadata(&p).ok();
                                (p, meta)
                            }).collect();

                        with_meta.sort_by_cached_key(|(_, meta)| {
                            meta.as_ref()
                                .and_then(|m| m.modified().ok())
                                .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
                        });

                        playlist = with_meta.into_iter().map(|(p, _)| p).collect();
                    }
                    SortMethod::DateCreated => {
                        let mut with_meta: Vec<(PathBuf, Option<std::fs::Metadata>)> =
                            playlist.into_iter().map(|p| {
                                let meta = std::fs::metadata(&p).ok();
                                (p, meta)
                            }).collect();

                        with_meta.sort_by_cached_key(|(_, meta)| {
                            meta.as_ref()
                                // Some OS/Filesystems don't support created(), fallback to modified
                                .and_then(|m| m.created().or_else(|_| m.modified()).ok())
                                .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
                        });

                        playlist = with_meta.into_iter().map(|(p, _)| p).collect();
                    }
                }

                if request.sort_order == SortOrder::Descending {
                    playlist.reverse();
                }

                if id_tracker.load(Ordering::Acquire) != request.request_id {
                    continue;
                }

                let _ = res_tx.send(DirectoryState {
                    request_id: request.request_id,
                    folder_path: folder.to_path_buf(),
                    playlist,
                });
            }
        }
    });

    (req_tx, res_rx)
}