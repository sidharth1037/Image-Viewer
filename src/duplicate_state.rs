use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::duplicate_scanner::{DuplicateScanRequest, DuplicateScanResult};
use crate::playlist_grid::PlaylistSelection;

/// One row of duplicate files in the duplicate finder view.
pub struct DuplicateGroup {
    /// The identical files in this group (≥ 2).
    pub paths: Vec<PathBuf>,
    /// Independent selection state for this row.
    pub selection: PlaylistSelection,
}

/// Overall state for the duplicate finder feature.
pub struct DuplicateFinderState {
    /// Groups of duplicate files found by the scanner.
    pub groups: Vec<DuplicateGroup>,
    /// True while a scan is in progress.
    pub scanning: bool,
    /// Cancellation token for the background scanner thread.
    pub scan_id: Arc<AtomicU64>,
    /// Channel to send scan requests to the background worker.
    pub scan_req_tx: Sender<DuplicateScanRequest>,
    /// Channel to receive scan results from the background worker.
    pub scan_res_rx: Receiver<DuplicateScanResult>,
    /// When entering Canvas mode from a duplicate row, tracks which group
    /// we came from so Escape returns to the duplicate finder.
    pub active_group_index: Option<usize>,
    /// Tracks the default playlist paths that were scanned in this session.
    pub last_scanned_paths: Option<Vec<PathBuf>>,
}

impl DuplicateFinderState {
    pub fn new(ctx: &eframe::egui::Context) -> Self {
        let scan_id = Arc::new(AtomicU64::new(0));
        let (scan_req_tx, scan_res_rx) =
            crate::duplicate_scanner::spawn_duplicate_scanner(scan_id.clone());
        let _ = ctx; // ctx reserved for future use (e.g. repaint on results)
        Self {
            groups: Vec::new(),
            scanning: false,
            scan_id,
            scan_req_tx,
            scan_res_rx,
            active_group_index: None,
            last_scanned_paths: None,
        }
    }

    /// Poll the result channel and populate groups when a scan finishes.
    pub fn process_scan_results(&mut self) -> bool {
        let current_id = self.scan_id.load(Ordering::Acquire);
        let mut got_results = false;
        while let Ok(result) = self.scan_res_rx.try_recv() {
            if result.request_id != current_id {
                continue;
            }
            self.groups = result
                .groups
                .into_iter()
                .map(|paths| DuplicateGroup {
                    paths,
                    selection: PlaylistSelection::default(),
                })
                .collect();
            self.scanning = false;
            got_results = true;
        }
        got_results
    }

    /// Start a new duplicate scan with the given file list.
    pub fn start_scan(&mut self, paths: Vec<PathBuf>) {
        self.last_scanned_paths = Some(paths.clone());
        let request_id = self.scan_id.fetch_add(1, Ordering::AcqRel) + 1;
        self.scanning = true;
        self.groups.clear();
        let _ = self.scan_req_tx.send(DuplicateScanRequest {
            paths,
            request_id,
        });
    }

    /// Remove the given paths from all groups and prune groups that drop below 2 members.
    pub fn remove_paths(&mut self, deleted: &[PathBuf]) {
        if let Some(ref mut paths) = self.last_scanned_paths {
            paths.retain(|p| !deleted.iter().any(|d| d == p));
        }
        for group in &mut self.groups {
            group.paths.retain(|p| !deleted.iter().any(|d| d == p));
            group.selection.clear();
        }
        // Prune groups that are no longer duplicates.
        self.groups.retain(|g| g.paths.len() >= 2);
    }

    /// Clear all state (e.g. when leaving duplicate finder mode).
    pub fn clear(&mut self) {
        self.scanning = false;
        self.active_group_index = None;
    }
}
