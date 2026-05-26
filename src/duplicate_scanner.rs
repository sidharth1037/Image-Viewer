use std::collections::HashMap;
use std::io::Read;
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use sha2::{Digest, Sha256};

use crate::duplicate_types::ScanResult;

/// Size of the partial-hash prefix in bytes.
const PARTIAL_HASH_SIZE: usize = 4096;

/// Exact-duplicate scan: size bucketing → partial SHA-256 → full SHA-256.
///
/// This is a pure scan function — no threading, no state management.
/// It streams results by sending `ScanResult` snapshots through `result_tx`
/// as size buckets are processed.
pub fn exact_scan(
    paths: &[PathBuf],
    cancel_token: &Arc<AtomicU64>,
    request_id: u64,
    result_tx: &Sender<ScanResult>,
) {
    let total_files = paths.len();

    // Step 1: Bucket by file size.
    let mut size_buckets: HashMap<u64, Vec<PathBuf>> = HashMap::new();

    for path in paths {
        if cancel_token.load(Ordering::Acquire) != request_id {
            return;
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
        let _ = result_tx.send(ScanResult {
            request_id,
            groups: Vec::new(),
            files_processed: total_files,
            total_files,
            is_complete: true,
        });
        return;
    }

    // Step 2: For each size bucket, compute partial hashes, then full hashes.
    // Stream results as each bucket is fully processed.
    let mut all_groups: Vec<Vec<PathBuf>> = Vec::new();
    let mut files_processed: usize;
    // Count files NOT in any size bucket (already eliminated).
    let files_in_buckets: usize = size_buckets.values().map(|v| v.len()).sum();
    files_processed = total_files - files_in_buckets;

    for bucket_files in size_buckets.values() {
        // Partial hash within this bucket.
        let mut partial_hash_map: HashMap<[u8; 32], Vec<PathBuf>> = HashMap::new();

        for path in bucket_files {
            if cancel_token.load(Ordering::Acquire) != request_id {
                return;
            }
            if let Some(hash) = partial_hash(path) {
                partial_hash_map
                    .entry(hash)
                    .or_default()
                    .push(path.clone());
            }
        }

        // Discard partial-hash groups with only one file.
        partial_hash_map.retain(|_, files| files.len() >= 2);

        // Full hash for remaining candidates.
        let mut full_hash_map: HashMap<[u8; 32], Vec<PathBuf>> = HashMap::new();

        for candidates in partial_hash_map.values() {
            for path in candidates {
                if cancel_token.load(Ordering::Acquire) != request_id {
                    return;
                }
                if let Some(hash) = full_hash(path) {
                    full_hash_map
                        .entry(hash)
                        .or_default()
                        .push(path.clone());
                }
            }
        }

        // Keep only groups with >= 2 identical files.
        for (_, group) in full_hash_map {
            if group.len() >= 2 {
                all_groups.push(group);
            }
        }

        files_processed += bucket_files.len();

        // Stream a snapshot after each bucket.
        // Sort groups for stable ordering.
        let mut snapshot = all_groups.clone();
        snapshot.sort_by(|a, b| a[0].cmp(&b[0]));

        let _ = result_tx.send(ScanResult {
            request_id,
            groups: snapshot,
            files_processed,
            total_files,
            is_complete: false,
        });
    }

    // Final result.
    all_groups.sort_by(|a, b| a[0].cmp(&b[0]));
    let _ = result_tx.send(ScanResult {
        request_id,
        groups: all_groups,
        files_processed: total_files,
        total_files,
        is_complete: true,
    });
}

use std::sync::{Mutex, OnceLock};

static PARTIAL_HASH_CACHE: OnceLock<Mutex<HashMap<PathBuf, [u8; 32]>>> = OnceLock::new();
static FULL_HASH_CACHE: OnceLock<Mutex<HashMap<PathBuf, [u8; 32]>>> = OnceLock::new();

fn get_partial_hash_cache() -> &'static Mutex<HashMap<PathBuf, [u8; 32]>> {
    PARTIAL_HASH_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn get_full_hash_cache() -> &'static Mutex<HashMap<PathBuf, [u8; 32]>> {
    FULL_HASH_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn partial_hash(path: &PathBuf) -> Option<[u8; 32]> {
    let cached = {
        let cache = get_partial_hash_cache().lock().unwrap();
        cache.get(path).cloned()
    };
    if let Some(hash) = cached {
        return Some(hash);
    }

    let mut file = std::fs::File::open(path).ok()?;
    let mut buffer = vec![0u8; PARTIAL_HASH_SIZE];
    let bytes_read = file.read(&mut buffer).ok()?;
    buffer.truncate(bytes_read);

    let mut hasher = Sha256::new();
    hasher.update(&buffer);
    let hash: [u8; 32] = hasher.finalize().into();

    let mut cache = get_partial_hash_cache().lock().unwrap();
    cache.insert(path.clone(), hash);
    Some(hash)
}

fn full_hash(path: &PathBuf) -> Option<[u8; 32]> {
    let cached = {
        let cache = get_full_hash_cache().lock().unwrap();
        cache.get(path).cloned()
    };
    if let Some(hash) = cached {
        return Some(hash);
    }

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

    let hash: [u8; 32] = hasher.finalize().into();

    let mut cache = get_full_hash_cache().lock().unwrap();
    cache.insert(path.clone(), hash);
    Some(hash)
}

