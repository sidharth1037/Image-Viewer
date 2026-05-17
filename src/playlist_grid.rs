use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use eframe::egui::{self, TextureHandle};

use crate::thumbnail_provider::{ThumbnailRequest, ThumbnailResult};

// ── Settings (endpoint for future configurability) ───────────────────────

/// Configuration for the playlist grid view.  Fields here are intentionally
/// public so a future settings UI can bind to them directly.
pub struct ThumbnailSettings {
    /// Desired width of each thumbnail cell in logical pixels.
    pub thumbnail_width: u32,
    /// Max thumbnail height as a multiple of the width.
    pub max_height_ratio: f32,
    /// Horizontal gap between items.
    pub item_spacing_x: f32,
    /// Vertical gap between rows.
    pub item_spacing_y: f32,
    /// Height reserved for the filename label below each thumbnail.
    pub label_height: f32,
}

impl Default for ThumbnailSettings {
    fn default() -> Self {
        Self {
            thumbnail_width: 160,
            max_height_ratio: 1.4,
            item_spacing_x: 8.0,
            item_spacing_y: 12.0,
            label_height: 20.0,
        }
    }
}

// ── Thumbnail cache entry ────────────────────────────────────────────────

pub enum ThumbnailEntry {
    Loading,
    Ready {
        texture: TextureHandle,
        width: u32,
        height: u32,
    },
    #[allow(dead_code)]
    Error(String),
}

// ── Selection state ──────────────────────────────────────────────────────

#[derive(Default)]
pub struct PlaylistSelection {
    /// Set of selected indices into the active playlist.
    pub selected: BTreeSet<usize>,
    /// Anchor for shift-click range selection.
    pub anchor: Option<usize>,
    /// Last clicked item (for keyboard navigation, future use).
    pub last_clicked: Option<usize>,
}

impl PlaylistSelection {
    pub fn clear(&mut self) {
        self.selected.clear();
        self.anchor = None;
        self.last_clicked = None;
    }

    /// Select a single item, clearing all others.
    pub fn select_single(&mut self, index: usize) {
        self.selected.clear();
        self.selected.insert(index);
        self.anchor = Some(index);
        self.last_clicked = Some(index);
    }

    /// Handle a click with modifier awareness.
    pub fn handle_click(&mut self, index: usize, ctrl: bool, shift: bool, total_items: usize) {
        if ctrl {
            // Toggle the clicked item.
            if self.selected.contains(&index) {
                self.selected.remove(&index);
            } else {
                self.selected.insert(index);
            }
            self.anchor = Some(index);
            self.last_clicked = Some(index);
        } else if shift {
            // Range select from anchor to index.
            let anchor = self.anchor.unwrap_or(0).min(total_items.saturating_sub(1));
            let (start, end) = if anchor <= index {
                (anchor, index)
            } else {
                (index, anchor)
            };
            self.selected.clear();
            for i in start..=end {
                self.selected.insert(i);
            }
            self.last_clicked = Some(index);
            // Anchor stays where it was.
        } else {
            self.select_single(index);
        }
    }

    pub fn is_selected(&self, index: usize) -> bool {
        self.selected.contains(&index)
    }
}

// ── Main grid state ──────────────────────────────────────────────────────

pub struct PlaylistGridState {
    /// Thumbnail texture cache, keyed by file path.
    pub thumbnail_cache: HashMap<PathBuf, ThumbnailEntry>,
    /// Paths that have been requested but not yet received.
    pub pending_requests: HashSet<PathBuf>,
    /// Selection state.
    pub selection: PlaylistSelection,
    /// View settings.
    pub settings: ThumbnailSettings,
    /// Scroll offset to restore on re-entry.
    pub scroll_to_index: Option<usize>,

    pub cached_total_size_bytes: u64,
    pub cached_selected_size_bytes: u64,

    // ── Thumbnail worker channels ──
    pub thumb_req_tx: Sender<ThumbnailRequest>,
    pub thumb_res_rx: Receiver<ThumbnailResult>,
    /// Epoch counter — incremented when we want to invalidate all in-flight
    /// requests (e.g. when the folder changes).
    pub thumb_epoch: Arc<AtomicU64>,
}

impl PlaylistGridState {
    pub fn new(ctx: &egui::Context) -> Self {
        let epoch = Arc::new(AtomicU64::new(1));
        let (req_tx, res_rx) =
            crate::thumbnail_provider::spawn_thumbnail_workers(4, ctx.clone(), epoch.clone());

        Self {
            thumbnail_cache: HashMap::new(),
            pending_requests: HashSet::new(),
            selection: PlaylistSelection::default(),
            settings: ThumbnailSettings::default(),
            scroll_to_index: None,
            cached_total_size_bytes: 0,
            cached_selected_size_bytes: 0,
            thumb_req_tx: req_tx,
            thumb_res_rx: res_rx,
            thumb_epoch: epoch,
        }
    }

    pub fn refresh_total_size_cache(&mut self, playlist: &[PathBuf]) {
        self.cached_total_size_bytes = playlist
            .iter()
            .filter_map(|path| std::fs::metadata(path).ok().map(|meta| meta.len()))
            .sum();
        self.refresh_selected_size_cache(playlist);
    }

    pub fn refresh_selected_size_cache(&mut self, playlist: &[PathBuf]) {
        self.cached_selected_size_bytes = self
            .selection
            .selected
            .iter()
            .filter_map(|index| playlist.get(*index))
            .filter_map(|path| std::fs::metadata(path).ok().map(|meta| meta.len()))
            .sum();
    }

    /// Drain the result channel and upload textures for completed thumbnails.
    pub fn process_thumbnail_results(&mut self, ctx: &egui::Context) {
        let current_epoch = self.thumb_epoch.load(Ordering::Acquire);

        while let Ok(result) = self.thumb_res_rx.try_recv() {
            // Discard stale results.
            if result.request_id != current_epoch {
                continue;
            }

            self.pending_requests.remove(&result.path);

            match result.result {
                Ok(img) => {
                    let color_image = egui::ColorImage::from_rgba_unmultiplied(
                        [img.width as usize, img.height as usize],
                        &img.rgba_pixels,
                    );
                    let texture = ctx.load_texture(
                        format!("thumb_{}", result.path.display()),
                        color_image,
                        egui::TextureOptions::LINEAR,
                    );
                    self.thumbnail_cache.insert(
                        result.path,
                        ThumbnailEntry::Ready {
                            texture,
                            width: img.width,
                            height: img.height,
                        },
                    );
                }
                Err(msg) => {
                    self.thumbnail_cache
                        .insert(result.path, ThumbnailEntry::Error(msg));
                }
            }
        }
    }

    /// Request thumbnails for items that are not yet cached or pending.
    pub fn request_thumbnails_for_paths(&mut self, paths: &[PathBuf]) {
        let epoch = self.thumb_epoch.load(Ordering::Acquire);
        let desired_size = self.settings.thumbnail_width;

        for path in paths {
            if self.thumbnail_cache.contains_key(path) || self.pending_requests.contains(path) {
                continue;
            }

            self.pending_requests.insert(path.clone());
            self.thumbnail_cache
                .insert(path.clone(), ThumbnailEntry::Loading);

            let _ = self.thumb_req_tx.send(ThumbnailRequest {
                path: path.clone(),
                desired_size,
                request_id: epoch,
            });
        }
    }

    /// Invalidate everything when the folder changes.
    pub fn clear_for_new_folder(&mut self) {
        // Bump the epoch so in-flight workers discard their results.
        self.thumb_epoch.fetch_add(1, Ordering::AcqRel);
        self.thumbnail_cache.clear();
        self.pending_requests.clear();
        self.selection.clear();
        self.scroll_to_index = None;
        self.cached_total_size_bytes = 0;
        self.cached_selected_size_bytes = 0;
    }
}
