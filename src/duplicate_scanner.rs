use std::collections::HashMap;
use std::io::Read;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use sha2::{Digest, Sha256};

/// Request to scan a list of files for duplicates.
pub struct DuplicateScanRequest {
    pub paths: Vec<PathBuf>,
    pub request_id: u64,
}

/// Result of a duplicate scan: groups of identical files (each group has ≥ 2 members).
pub struct DuplicateScanResult {
    pub request_id: u64,
    pub groups: Vec<Vec<PathBuf>>,
}

/// Size of the partial-hash prefix in bytes.
const PARTIAL_HASH_SIZE: usize = 4096;

pub fn spawn_duplicate_scanner(
    cancel_token: Arc<AtomicU64>,
) -> (Sender<DuplicateScanRequest>, Receiver<DuplicateScanResult>) {
    let (req_tx, req_rx) = channel::<DuplicateScanRequest>();
    let (res_tx, res_rx) = channel::<DuplicateScanResult>();

    std::thread::spawn(move || {
        while let Ok(mut request) = req_rx.recv() {
            // Drain to keep only the latest request.
            while let Ok(newer) = req_rx.try_recv() {
                request = newer;
            }

            if cancel_token.load(Ordering::Acquire) != request.request_id {
                continue;
            }

            let groups = find_duplicates(&request.paths, &cancel_token, request.request_id);

            if cancel_token.load(Ordering::Acquire) != request.request_id {
                continue;
            }

            let _ = res_tx.send(DuplicateScanResult {
                request_id: request.request_id,
                groups,
            });
        }
    });

    (req_tx, res_rx)
}

fn find_duplicates(
    paths: &[PathBuf],
    cancel_token: &Arc<AtomicU64>,
    request_id: u64,
) -> Vec<Vec<PathBuf>> {
    // Step 1: Bucket by file size.
    let mut size_buckets: HashMap<u64, Vec<PathBuf>> = HashMap::new();

    for path in paths {
        if cancel_token.load(Ordering::Acquire) != request_id {
            return Vec::new();
        }
        if let Ok(meta) = std::fs::metadata(path) {
            if meta.is_file() {
                size_buckets
                    .entry(meta.len())
                    .or_default()
                    .push(path.clone());
            }
        }
    }

    // Discard size buckets with only one file.
    size_buckets.retain(|_, files| files.len() >= 2);

    if size_buckets.is_empty() {
        return Vec::new();
    }

    // Step 2: For each size bucket, compute partial hashes.
    let mut partial_hash_buckets: HashMap<(u64, [u8; 32]), Vec<PathBuf>> = HashMap::new();

    for (size, files) in &size_buckets {
        for path in files {
            if cancel_token.load(Ordering::Acquire) != request_id {
                return Vec::new();
            }
            if let Some(hash) = partial_hash(path) {
                partial_hash_buckets
                    .entry((*size, hash))
                    .or_default()
                    .push(path.clone());
            }
        }
    }

    // Discard partial-hash buckets with only one file.
    partial_hash_buckets.retain(|_, files| files.len() >= 2);

    if partial_hash_buckets.is_empty() {
        return Vec::new();
    }

    // Step 3: For remaining candidates, compute full file hash.
    let mut full_hash_buckets: HashMap<[u8; 32], Vec<PathBuf>> = HashMap::new();

    for (_, files) in &partial_hash_buckets {
        for path in files {
            if cancel_token.load(Ordering::Acquire) != request_id {
                return Vec::new();
            }
            if let Some(hash) = full_hash(path) {
                full_hash_buckets
                    .entry(hash)
                    .or_default()
                    .push(path.clone());
            }
        }
    }

    // Keep only groups with ≥ 2 identical files.
    full_hash_buckets.retain(|_, files| files.len() >= 2);

    // Convert to Vec<Vec<PathBuf>>, sorted by first path in each group for stable ordering.
    let mut groups: Vec<Vec<PathBuf>> = full_hash_buckets.into_values().collect();
    groups.sort_by(|a, b| a[0].cmp(&b[0]));
    groups
}

fn partial_hash(path: &PathBuf) -> Option<[u8; 32]> {
    let mut file = std::fs::File::open(path).ok()?;
    let mut buffer = vec![0u8; PARTIAL_HASH_SIZE];
    let bytes_read = file.read(&mut buffer).ok()?;
    buffer.truncate(bytes_read);

    let mut hasher = Sha256::new();
    hasher.update(&buffer);
    Some(hasher.finalize().into())
}

fn full_hash(path: &PathBuf) -> Option<[u8; 32]> {
    let mut file = std::fs::File::open(path).ok()?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];

    loop {
        let bytes_read = file.read(&mut buffer).ok()?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    Some(hasher.finalize().into())
}
