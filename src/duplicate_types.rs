use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// Identifies which duplicate detection method is active.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ScanType {
    /// SHA-256 byte-identical duplicates.
    Exact,
    /// Visual similarity via perceptual image hashing.
    Perceptual,
}

impl ScanType {
    /// Human-readable label for UI display.
    pub fn label(&self) -> &'static str {
        match self {
            ScanType::Exact => "Same File",
            ScanType::Perceptual => "Similar Image",
        }
    }
}

/// Request sent to a background scanner thread.
pub struct ScanRequest {
    pub paths: Vec<PathBuf>,
    pub request_id: u64,
}

/// Result streamed from a background scanner thread.
/// Each result is a FULL SNAPSHOT of all groups found so far.
pub struct ScanResult {
    pub request_id: u64,
    /// Complete snapshot of all duplicate groups found so far.
    pub groups: Vec<Vec<PathBuf>>,
    /// Number of files processed so far (for progress display).
    pub files_processed: usize,
    /// Total number of files to process.
    pub total_files: usize,
    /// True if this is the final result (scan finished).
    pub is_complete: bool,
}

/// Generic scanner spawner. Wraps ANY scan function in a background
/// thread with channels and cancellation.
///
/// The scan function streams results by sending `ScanResult` messages
/// through the provided `Sender`. It has no knowledge of threading.
///
/// # Arguments
/// * `cancel_token` — Shared atomic for cancellation. The scan function
///   should check this periodically and abort if it doesn't match.
/// * `scan_fn` — The scan algorithm as a pure function:
///   `fn(paths, cancel_token, request_id, result_sender)`
pub fn spawn_scanner<F>(
    cancel_token: Arc<AtomicU64>,
    scan_fn: F,
) -> (Sender<ScanRequest>, Receiver<ScanResult>)
where
    F: Fn(&[PathBuf], &Arc<AtomicU64>, u64, &Sender<ScanResult>) + Send + 'static,
{
    let (req_tx, req_rx) = channel::<ScanRequest>();
    let (res_tx, res_rx) = channel::<ScanResult>();

    std::thread::spawn(move || {
        while let Ok(mut request) = req_rx.recv() {
            // Drain to keep only the latest request.
            while let Ok(newer) = req_rx.try_recv() {
                request = newer;
            }

            if cancel_token.load(Ordering::Acquire) != request.request_id {
                continue;
            }

            scan_fn(&request.paths, &cancel_token, request.request_id, &res_tx);
        }
    });

    (req_tx, res_rx)
}
