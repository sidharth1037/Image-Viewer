use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender};

use crate::image_io::{LoadFailure, LoadedImage};

const MB: usize = 1024 * 1024;
const MIN_ADAPTIVE_BUDGET: usize = 64 * MB;
const MAX_ADAPTIVE_BUDGET: usize = 512 * MB;
const DEFAULT_ADAPTIVE_BUDGET: usize = 192 * MB;

#[derive(Clone, Copy)]
enum Slot {
    Next1,
    Next2,
}

struct PendingJob {
    slot: Slot,
    path: PathBuf,
    epoch: u64,
}

struct CacheEntry {
    path: PathBuf,
    image: LoadedImage,
    decoded_bytes: usize,
}

impl CacheEntry {
    fn new(path: PathBuf, image: LoadedImage) -> Self {
        let decoded_bytes = image.frames.iter().map(|f| f.pixels.len()).sum();
        Self {
            path,
            image,
            decoded_bytes,
        }
    }
}

pub struct PreloadRing {
    preload_epoch: Arc<AtomicU64>,
    req_tx: Sender<(PathBuf, u64)>,
    res_rx: Receiver<Result<LoadedImage, LoadFailure>>,

    pending: VecDeque<PendingJob>,

    current: Option<CacheEntry>,
    prev: Option<CacheEntry>,
    next1: Option<CacheEntry>,
    next2: Option<CacheEntry>,

    instant_current: Option<LoadedImage>,
}

impl PreloadRing {
    pub fn new(
        preload_epoch: Arc<AtomicU64>,
        req_tx: Sender<(PathBuf, u64)>,
        res_rx: Receiver<Result<LoadedImage, LoadFailure>>,
    ) -> Self {
        Self {
            preload_epoch,
            req_tx,
            res_rx,
            pending: VecDeque::new(),
            current: None,
            prev: None,
            next1: None,
            next2: None,
            instant_current: None,
        }
    }

    pub fn on_new_open(&mut self) {
        self.bump_epoch();
        self.pending.clear();
        self.current = None;
        self.prev = None;
        self.next1 = None;
        self.next2 = None;
        self.instant_current = None;
    }

    pub fn on_navigation_away(&mut self, direction: i32) {
        if let Some(current) = self.current.take() {
            if direction < 0 {
                // Moving backward: keep the outgoing image in the forward cache path.
                self.next2 = self.next1.take();
                self.next1 = Some(current);
            } else {
                // Moving forward (or unknown): keep the outgoing image as immediate previous.
                self.prev = Some(current);
            }
        }
    }

    pub fn process_worker_results(&mut self) {
        while let Ok(result) = self.res_rx.try_recv() {
            let Some(job) = self.pending.pop_front() else {
                continue;
            };

            if job.epoch != self.current_epoch() {
                continue;
            }

            match result {
                Ok(loaded) => {
                    let entry = CacheEntry::new(job.path, loaded);
                    match job.slot {
                        Slot::Next1 => self.next1 = Some(entry),
                        Slot::Next2 => self.next2 = Some(entry),
                    }
                }
                Err(_) => {
                    // Preload failures are isolated; foreground navigation may retry decode.
                }
            }
        }

        self.enforce_budget();
    }

    pub fn try_take_cached_for_path(&mut self, path: &PathBuf) -> Option<LoadedImage> {
        if self.current.as_ref().is_some_and(|e| &e.path == path) {
            return self.current.take().map(|e| e.image);
        }
        if self.prev.as_ref().is_some_and(|e| &e.path == path) {
            return self.prev.take().map(|e| e.image);
        }
        if self.next1.as_ref().is_some_and(|e| &e.path == path) {
            return self.next1.take().map(|e| e.image);
        }
        if self.next2.as_ref().is_some_and(|e| &e.path == path) {
            return self.next2.take().map(|e| e.image);
        }
        None
    }

    pub fn set_instant_current(&mut self, image: LoadedImage) {
        self.instant_current = Some(image);
    }

    pub fn take_instant_current(&mut self) -> Option<LoadedImage> {
        self.instant_current.take()
    }

    pub fn on_playlist_updated(
        &mut self,
        playlist: &[PathBuf],
        current_index: usize,
        loop_playlist: bool,
        current_path: Option<&PathBuf>,
    ) {
        self.bump_epoch();
        self.pending.clear();
        self.next1 = None;
        self.next2 = None;

        if let Some(path) = current_path {
            if self.current.as_ref().is_none_or(|e| &e.path != path) {
                self.current = None;
            }
        } else {
            self.current = None;
        }

        self.schedule_next_two(playlist, current_index, loop_playlist);
    }

    pub fn on_current_image_ready(
        &mut self,
        path: PathBuf,
        current_index: usize,
        image: LoadedImage,
        playlist: &[PathBuf],
        loop_playlist: bool,
    ) {
        self.current = Some(CacheEntry::new(path, image));
        self.schedule_next_two(playlist, current_index, loop_playlist);
        self.enforce_budget();
    }

    fn schedule_next_two(&mut self, playlist: &[PathBuf], current_index: usize, loop_playlist: bool) {
        self.bump_epoch();
        self.pending.clear();
        self.next1 = None;
        self.next2 = None;

        let targets = next_indices(playlist.len(), current_index, loop_playlist, 2);
        if targets.is_empty() {
            return;
        }

        let epoch = self.current_epoch();

        for (slot, idx) in [(Slot::Next1, 0usize), (Slot::Next2, 1usize)] {
            if idx >= targets.len() {
                continue;
            }

            let target_index = targets[idx];
            let target_path = playlist[target_index].clone();

            if self.prev.as_ref().is_some_and(|e| e.path == target_path)
                || self.current.as_ref().is_some_and(|e| e.path == target_path)
            {
                continue;
            }

            let _ = self.req_tx.send((target_path.clone(), epoch));
            self.pending.push_back(PendingJob {
                slot,
                path: target_path,
                epoch,
            });
        }
    }

    fn adaptive_budget_bytes(&self) -> usize {
        if let Some(current) = &self.current {
            (current.decoded_bytes.saturating_mul(4)).clamp(MIN_ADAPTIVE_BUDGET, MAX_ADAPTIVE_BUDGET)
        } else {
            DEFAULT_ADAPTIVE_BUDGET
        }
    }

    fn total_cached_bytes(&self) -> usize {
        self.current.as_ref().map_or(0, |e| e.decoded_bytes)
            + self.prev.as_ref().map_or(0, |e| e.decoded_bytes)
            + self.next1.as_ref().map_or(0, |e| e.decoded_bytes)
            + self.next2.as_ref().map_or(0, |e| e.decoded_bytes)
    }

    fn enforce_budget(&mut self) {
        let budget = self.adaptive_budget_bytes();
        while self.total_cached_bytes() > budget {
            if self.next2.take().is_some() {
                continue;
            }
            if self.next1.take().is_some() {
                continue;
            }
            if self.prev.take().is_some() {
                continue;
            }
            self.current = None;
            break;
        }
    }

    fn bump_epoch(&self) {
        self.preload_epoch.fetch_add(1, Ordering::AcqRel);
    }

    fn current_epoch(&self) -> u64 {
        self.preload_epoch.load(Ordering::Acquire)
    }
}

fn next_indices(len: usize, current_index: usize, loop_playlist: bool, max_count: usize) -> Vec<usize> {
    if len <= 1 || max_count == 0 {
        return Vec::new();
    }

    let mut out = Vec::new();

    if loop_playlist {
        for step in 1..=(max_count * 2) {
            let idx = (current_index + step) % len;
            if idx == current_index || out.contains(&idx) {
                continue;
            }
            out.push(idx);
            if out.len() == max_count {
                break;
            }
        }
    } else {
        for step in 1..=max_count {
            let idx = current_index + step;
            if idx < len {
                out.push(idx);
            }
        }
    }

    out
}