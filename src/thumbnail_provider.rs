use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// Raw RGBA pixels extracted from a thumbnail.
pub struct ThumbnailImage {
    pub rgba_pixels: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

pub struct ThumbnailRequest {
    pub path: PathBuf,
    pub desired_size: u32,
    pub request_id: u64,
}

pub struct ThumbnailResult {
    pub path: PathBuf,
    pub request_id: u64,
    pub result: Result<ThumbnailImage, String>,
}

/// Spawns `worker_count` background threads for thumbnail extraction.
///
/// Each thread initialises COM and uses `IShellItemImageFactory::GetImage` to
/// obtain thumbnails from Windows' own thumbnail cache (or to generate one on
/// the fly).  Results are posted back to the returned receiver.
pub fn spawn_thumbnail_workers(
    worker_count: usize,
    ctx: eframe::egui::Context,
    epoch: Arc<AtomicU64>,
) -> (Sender<ThumbnailRequest>, Receiver<ThumbnailResult>) {
    let (req_tx, req_rx) = channel::<ThumbnailRequest>();
    let (res_tx, res_rx) = channel::<ThumbnailResult>();

    // Wrap the receiver in an Arc<Mutex<>> so multiple workers can share it.
    let req_rx = Arc::new(std::sync::Mutex::new(req_rx));

    for _ in 0..worker_count {
        let req_rx = Arc::clone(&req_rx);
        let res_tx = res_tx.clone();
        let ctx = ctx.clone();
        let epoch = Arc::clone(&epoch);

        std::thread::spawn(move || {
            worker_loop(&req_rx, &res_tx, &ctx, &epoch);
        });
    }

    (req_tx, res_rx)
}

fn worker_loop(
    req_rx: &Arc<std::sync::Mutex<Receiver<ThumbnailRequest>>>,
    res_tx: &Sender<ThumbnailResult>,
    ctx: &eframe::egui::Context,
    epoch: &Arc<AtomicU64>,
) {
    #[cfg(windows)]
    {
        // Each worker thread must initialise COM independently.
        unsafe {
            use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        }
    }

    loop {
        let request = {
            let rx = match req_rx.lock() {
                Ok(rx) => rx,
                Err(_) => return, // Mutex poisoned — exit gracefully.
            };
            match rx.recv() {
                Ok(req) => req,
                Err(_) => return, // Channel closed — exit.
            }
        };

        // Check cancellation: if the epoch has changed since this request was
        // enqueued, skip it silently.
        if epoch.load(Ordering::Acquire) != request.request_id {
            continue;
        }

        let result = extract_thumbnail(&request.path, request.desired_size);

        // Re-check epoch before sending (avoid queueing stale results).
        if epoch.load(Ordering::Acquire) != request.request_id {
            continue;
        }

        let _ = res_tx.send(ThumbnailResult {
            path: request.path,
            request_id: request.request_id,
            result,
        });

        ctx.request_repaint();
    }
}

// ── Platform-specific thumbnail extraction ────────────────────────────────

#[cfg(windows)]
fn extract_thumbnail(path: &std::path::Path, desired_size: u32) -> Result<ThumbnailImage, String> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::{Interface, PCWSTR};
    use windows::Win32::Graphics::Gdi::*;
    use windows::Win32::UI::Shell::*;

    let wide_path: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    unsafe {
        // 1. Create IShellItem from file path.
        let shell_item: windows::Win32::UI::Shell::IShellItem =
            SHCreateItemFromParsingName(PCWSTR(wide_path.as_ptr()), None)
                .map_err(|e| format!("SHCreateItemFromParsingName failed: {}", e))?;

        // 2. Query for IShellItemImageFactory.
        let image_factory: IShellItemImageFactory = shell_item
            .cast()
            .map_err(|e| format!("IShellItemImageFactory cast failed: {}", e))?;

        // 3. Request the thumbnail. SIIGBF_RESIZETOFIT preserves aspect ratio.
        let size = windows::Win32::Foundation::SIZE {
            cx: desired_size as i32,
            cy: desired_size as i32,
        };
        let hbitmap = image_factory
            .GetImage(size, SIIGBF_RESIZETOFIT)
            .map_err(|e| format!("GetImage failed: {}", e))?;

        // 4. Convert HBITMAP → RGBA pixels.
        let result = hbitmap_to_rgba(hbitmap);

        // 5. Clean up the HBITMAP.
        let _ = DeleteObject(hbitmap.into());

        result
    }
}

#[cfg(windows)]
fn hbitmap_to_rgba(
    hbitmap: windows::Win32::Graphics::Gdi::HBITMAP,
) -> Result<ThumbnailImage, String> {
    use windows::Win32::Graphics::Gdi::*;

    let mut bmp = BITMAP::default();
    let got = unsafe {
        GetObjectW(
            hbitmap.into(),
            std::mem::size_of::<BITMAP>() as i32,
            Some(&mut bmp as *mut BITMAP as *mut _),
        )
    };
    if got == 0 {
        return Err("GetObject failed on HBITMAP".into());
    }

    let width = bmp.bmWidth;
    let height = bmp.bmHeight;
    if width <= 0 || height <= 0 {
        return Err(format!("Invalid bitmap dimensions: {}x{}", width, height));
    }

    let mut bmi = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width,
            biHeight: -height,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0 as u32,
            ..Default::default()
        },
        ..Default::default()
    };

    let pixel_count = (width * height * 4) as usize;
    let mut bgra_pixels = vec![0u8; pixel_count];

    let hdc = unsafe { CreateCompatibleDC(None) };
    let rows = unsafe {
        GetDIBits(
            hdc,
            hbitmap,
            0,
            height as u32,
            Some(bgra_pixels.as_mut_ptr() as *mut _),
            &mut bmi,
            DIB_RGB_COLORS,
        )
    };
    let _ = unsafe { DeleteDC(hdc) };

    if rows == 0 {
        return Err("GetDIBits returned 0 rows".into());
    }

    for chunk in bgra_pixels.chunks_exact_mut(4) {
        chunk.swap(0, 2);
        if chunk[3] == 0 {
            chunk[3] = 255;
        }
    }

    Ok(ThumbnailImage {
        rgba_pixels: bgra_pixels,
        width: width as u32,
        height: height as u32,
    })
}

#[cfg(not(windows))]
fn extract_thumbnail(_path: &std::path::Path, _desired_size: u32) -> Result<ThumbnailImage, String> {
    Err("Thumbnail extraction is only supported on Windows".into())
}
