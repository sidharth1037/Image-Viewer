use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use image_hasher::{HashAlg, HasherConfig};

use crate::duplicate_types::ScanResult;

// ── Tunable constants ───────────────────────────────────────────────────────

/// Hamming distance threshold for perceptual similarity.
/// Lower = stricter matching.
/// 0 = visually identical hashes
/// 1-3 = same image, different quality/resolution
/// 4-7 = somewhat similar
/// 8+ = different images
const PERCEPTUAL_THRESHOLD: u32 = 3;

/// Hash algorithm. Gradient (dHash) is a good balance of speed and accuracy.
const HASH_ALGORITHM: HashAlg = HashAlg::Gradient;

/// Hash dimensions (width × height in bits). 8×8 = 64-bit hash.
const HASH_WIDTH: u32 = 8;
const HASH_HEIGHT: u32 = 8;

/// Send a progress snapshot every N images hashed.
const PROGRESS_BATCH_SIZE: usize = 20;

// ── Union-Find ──────────────────────────────────────────────────────────────

/// Disjoint Set Union (Union-Find) with path compression and union by rank.
struct UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
}

impl UnionFind {
    fn new(n: usize) -> Self {
        Self {
            parent: (0..n).collect(),
            rank: vec![0; n],
        }
    }

    fn find(&mut self, x: usize) -> usize {
        if self.parent[x] != x {
            self.parent[x] = self.find(self.parent[x]);
        }
        self.parent[x]
    }

    fn union(&mut self, x: usize, y: usize) {
        let rx = self.find(x);
        let ry = self.find(y);
        if rx == ry {
            return;
        }
        match self.rank[rx].cmp(&self.rank[ry]) {
            std::cmp::Ordering::Less => self.parent[rx] = ry,
            std::cmp::Ordering::Greater => self.parent[ry] = rx,
            std::cmp::Ordering::Equal => {
                self.parent[ry] = rx;
                self.rank[rx] += 1;
            }
        }
    }
}

// ── Scan function ───────────────────────────────────────────────────────────

/// Perceptual-duplicate scan: image hashing → pairwise comparison → union-find grouping.
///
/// This is a pure scan function — no threading, no state management.
/// It streams results by sending `ScanResult` snapshots through `result_tx`
/// every `PROGRESS_BATCH_SIZE` images hashed.
pub fn perceptual_scan(
    paths: &[PathBuf],
    cancel_token: &Arc<AtomicU64>,
    request_id: u64,
    result_tx: &Sender<ScanResult>,
) {
    let total_files = paths.len();

    if total_files < 2 {
        let _ = result_tx.send(ScanResult {
            request_id,
            groups: Vec::new(),
            files_processed: total_files,
            total_files,
            is_complete: true,
        });
        return;
    }

    let hasher = HasherConfig::new()
        .hash_alg(HASH_ALGORITHM)
        .hash_size(HASH_WIDTH, HASH_HEIGHT)
        .to_hasher();

    // Phase 1: Compute perceptual hash for each image.
    // Store (original_index, hash) for images that decode successfully.
    let mut hashes: Vec<(usize, image_hasher::ImageHash)> = Vec::new();
    let mut uf = UnionFind::new(paths.len());

    for (i, path) in paths.iter().enumerate() {
        if cancel_token.load(Ordering::Acquire) != request_id {
            return;
        }

        if let Ok(img) = image::open(path) {
            let new_hash = hasher.hash_image(&img);

            // Compare against all previously hashed images immediately.
            for &(prev_idx, ref prev_hash) in &hashes {
                if new_hash.dist(prev_hash) <= PERCEPTUAL_THRESHOLD {
                    uf.union(i, prev_idx);
                }
            }

            hashes.push((i, new_hash));
        }

        // Send progress snapshot periodically.
        if (i + 1) % PROGRESS_BATCH_SIZE == 0 || i + 1 == total_files {
            let groups = extract_groups(&mut uf, &hashes, paths);
            let _ = result_tx.send(ScanResult {
                request_id,
                groups,
                files_processed: i + 1,
                total_files,
                is_complete: false,
            });
        }
    }

    // Final result.
    let groups = extract_groups(&mut uf, &hashes, paths);
    let _ = result_tx.send(ScanResult {
        request_id,
        groups,
        files_processed: total_files,
        total_files,
        is_complete: true,
    });
}

/// Extract duplicate groups from the Union-Find structure.
fn extract_groups(
    uf: &mut UnionFind,
    hashes: &[(usize, image_hasher::ImageHash)],
    paths: &[PathBuf],
) -> Vec<Vec<PathBuf>> {
    let mut components: HashMap<usize, Vec<PathBuf>> = HashMap::new();
    for &(idx, _) in hashes {
        let root = uf.find(idx);
        components
            .entry(root)
            .or_default()
            .push(paths[idx].clone());
    }

    let mut groups: Vec<Vec<PathBuf>> = components
        .into_values()
        .filter(|g| g.len() >= 2)
        .collect();

    // Sort groups by first path for stable ordering.
    groups.sort_by(|a, b| a[0].cmp(&b[0]));
    groups
}
