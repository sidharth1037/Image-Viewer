use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::duplicate_types::{ScanRequest, ScanResult, ScanType};
use crate::playlist_grid::PlaylistSelection;

/// One row of duplicate files in the duplicate finder view.
pub struct DuplicateGroup {
    /// The files in this group (≥ 2).
    pub paths: Vec<PathBuf>,
    /// Independent selection state for this row.
    pub selection: PlaylistSelection,
}

// ── ScanState ───────────────────────────────────────────────────────────────

/// Per-scan-type state. Manages groups, scanning flag, progress,
/// channels, cancel token, and session cache.
///
/// This struct is completely independent of the scan algorithm used.
/// It works identically for exact and perceptual scans.
pub struct ScanState {
    /// Groups of duplicate files found by this scan.
    pub groups: Vec<DuplicateGroup>,
    /// True while a scan is in progress.
    pub scanning: bool,
    /// Number of files processed so far (for progress display).
    pub files_processed: usize,
    /// Total number of files being scanned.
    pub total_files: usize,
    /// Cancellation / versioning token for this scan type.
    scan_id: Arc<AtomicU64>,
    /// Channel to send scan requests to the background worker.
    req_tx: Sender<ScanRequest>,
    /// Channel to receive scan results from the background worker.
    res_rx: Receiver<ScanResult>,
    /// Tracks the paths that were scanned in the last run (session cache).
    last_scanned_paths: Option<Vec<PathBuf>>,
}

impl ScanState {
    /// Create a new `ScanState` from pre-built channels and cancel token.
    pub fn new(
        scan_id: Arc<AtomicU64>,
        req_tx: Sender<ScanRequest>,
        res_rx: Receiver<ScanResult>,
    ) -> Self {
        Self {
            groups: Vec::new(),
            scanning: false,
            files_processed: 0,
            total_files: 0,
            scan_id,
            req_tx,
            res_rx,
            last_scanned_paths: None,
        }
    }

    /// Start a scan if the path set has changed since the last scan.
    /// Returns `true` if a new scan was triggered.
    pub fn start_scan(&mut self, paths: Vec<PathBuf>) -> bool {
        let needs_scan = self
            .last_scanned_paths
            .as_ref()
            .map_or(true, |last| *last != paths);
        if needs_scan {
            self.force_scan(paths);
            true
        } else {
            false
        }
    }

    /// Always start a scan, ignoring the session cache.
    pub fn force_scan(&mut self, paths: Vec<PathBuf>) {
        self.last_scanned_paths = Some(paths.clone());
        let request_id = self.scan_id.fetch_add(1, Ordering::AcqRel) + 1;
        self.scanning = true;
        self.groups.clear();
        self.files_processed = 0;
        self.total_files = paths.len();
        let _ = self.req_tx.send(ScanRequest {
            paths,
            request_id,
        });
    }

    /// Poll the result channel and apply the latest snapshot.
    /// Returns `true` if state changed (new data to display).
    pub fn process_results(&mut self) -> bool {
        let current_id = self.scan_id.load(Ordering::Acquire);
        let mut latest: Option<ScanResult> = None;

        // Drain the channel, keep only the most recent valid result.
        while let Ok(result) = self.res_rx.try_recv() {
            if result.request_id == current_id {
                latest = Some(result);
            }
        }

        if let Some(result) = latest {
            // Full snapshot replacement.
            self.groups = result
                .groups
                .into_iter()
                .map(|paths| DuplicateGroup {
                    paths,
                    selection: PlaylistSelection::default(),
                })
                .collect();
            self.files_processed = result.files_processed;
            self.total_files = result.total_files;
            if result.is_complete {
                self.scanning = false;
            }
            return true;
        }
        false
    }

    /// Remove deleted paths from all groups and the session cache.
    /// Prunes groups that drop below 2 members.
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

    /// Clear transient state (e.g. when leaving duplicate finder mode).
    /// Keeps the session cache so re-entering skips redundant scans.
    pub fn clear(&mut self) {
        self.scanning = false;
    }

    /// Progress as a fraction in [0.0, 1.0], or `None` if not scanning.
    pub fn progress_fraction(&self) -> Option<f32> {
        if self.scanning && self.total_files > 0 {
            Some(self.files_processed as f32 / self.total_files as f32)
        } else {
            None
        }
    }
}

// ── DuplicateFinderState ────────────────────────────────────────────────────

/// Overall state for the duplicate finder feature.
/// Holds two independent `ScanState` instances — one for exact duplicates,
/// one for perceptually similar images.
pub struct DuplicateFinderState {
    /// Which tab is currently active in the UI.
    pub active_tab: ScanType,
    /// Exact (SHA-256) duplicate scan state.
    pub exact: ScanState,
    /// Perceptual (image hash) duplicate scan state.
    pub perceptual: ScanState,
    /// When entering Canvas mode from a duplicate row, tracks which group
    /// we came from so Escape returns to the duplicate finder.
    pub active_group_index: Option<usize>,
}

impl DuplicateFinderState {
    /// Create a new `DuplicateFinderState`, spawning both background scanner threads.
    pub fn new(_ctx: &eframe::egui::Context) -> Self {
        // Exact scanner.
        let exact_cancel = Arc::new(AtomicU64::new(0));
        let (exact_tx, exact_rx) = crate::duplicate_types::spawn_scanner(
            exact_cancel.clone(),
            crate::duplicate_scanner::exact_scan,
        );

        // Perceptual scanner.
        let perceptual_cancel = Arc::new(AtomicU64::new(0));
        let (perceptual_tx, perceptual_rx) = crate::duplicate_types::spawn_scanner(
            perceptual_cancel.clone(),
            crate::perceptual_scanner::perceptual_scan,
        );

        Self {
            active_tab: ScanType::Exact,
            exact: ScanState::new(exact_cancel, exact_tx, exact_rx),
            perceptual: ScanState::new(perceptual_cancel, perceptual_tx, perceptual_rx),
            active_group_index: None,
        }
    }

    /// Get a reference to the active tab's scan state.
    pub fn active_scan(&self) -> &ScanState {
        match self.active_tab {
            ScanType::Exact => &self.exact,
            ScanType::Perceptual => &self.perceptual,
        }
    }

    /// Get a mutable reference to the active tab's scan state.
    pub fn active_scan_mut(&mut self) -> &mut ScanState {
        match self.active_tab {
            ScanType::Exact => &mut self.exact,
            ScanType::Perceptual => &mut self.perceptual,
        }
    }

    /// Start both scans with the same path set.
    /// Each scan independently checks its session cache.
    pub fn start_all_scans(&mut self, paths: Vec<PathBuf>) {
        self.exact.start_scan(paths.clone());
        self.perceptual.start_scan(paths);
    }

    /// Poll both result channels. Returns `true` if any state changed.
    pub fn process_all_results(&mut self) -> bool {
        let a = self.exact.process_results();
        let b = self.perceptual.process_results();
        a || b
    }

    /// Remove deleted paths from both scan states.
    pub fn remove_paths_all(&mut self, deleted: &[PathBuf]) {
        self.exact.remove_paths(deleted);
        self.perceptual.remove_paths(deleted);
    }

    /// Whether either scan is currently in progress.
    pub fn any_scanning(&self) -> bool {
        self.exact.scanning || self.perceptual.scanning
    }

    /// Clear transient state for both scans.
    pub fn clear(&mut self) {
        self.exact.clear();
        self.perceptual.clear();
        self.active_group_index = None;
    }
}
