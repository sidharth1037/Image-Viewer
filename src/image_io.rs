use eframe::egui;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
// Added for versioning logic
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// Represents a single frame of an image, ready for GPU upload.
pub struct ImageFrame {
    pub pixels: Vec<u8>,
    pub duration_ms: u32,
}

/// The final payload sent from the background decoding thread to the UI thread.
pub struct LoadedImage {
    pub request_id: u64,
    pub width: u32,
    pub height: u32,
    pub density: Option<ImageDensity>,
    pub frames: Vec<ImageFrame>, // Holds 1 frame for static images, many for GIFs
}

#[derive(Clone, Copy)]
pub enum DensitySource {
    Exif,
    Container,
}

#[derive(Clone, Copy)]
pub struct ImageDensity {
    pub ppi_x: f32,
    pub ppi_y: f32,
    pub source: DensitySource,
}

impl ImageDensity {
    pub fn average_ppi(self) -> f32 {
        ((self.ppi_x + self.ppi_y) * 0.5).max(0.0001)
    }
}

pub struct LoadFailure {
    pub request_id: u64,
    pub message: String,
}

const MIN_ANIM_FRAME_MS: u32 = 16;

/// Helper function to rapidly convert 3-channel RGB data into 4-channel RGBA data.
/// eframe/egui strictly requires an Alpha channel for texture rendering.
fn pad_rgb_to_rgba(rgb_pixels: &[u8]) -> Vec<u8> {
    let mut rgba_pixels = Vec::with_capacity((rgb_pixels.len() / 3) * 4);
    for chunk in rgb_pixels.chunks_exact(3) {
        rgba_pixels.extend_from_slice(&[chunk[0], chunk[1], chunk[2], 255]);
    }
    rgba_pixels
}

fn looks_like_heif(file_bytes: &[u8]) -> bool {
    if file_bytes.len() < 12 || &file_bytes[4..8] != b"ftyp" {
        return false;
    }

    matches!(
        &file_bytes[8..12],
        b"heic" | b"heix" | b"hevc" | b"hevx" | b"mif1" | b"mif2"
    )
}

/// --- In-Memory Pixel Rotator ---
/// Takes a flat RGBA pixel array and physically rearranges the bytes based on EXIF rules.
fn apply_exif_orientation(
    pixels: Vec<u8>,
    width: u32,
    height: u32,
    orientation: u32,
) -> (u32, u32, Vec<u8>) {
    // If orientation is 1 (Normal) or invalid, return the original array without cloning (Zero cost!)
    if orientation <= 1 || orientation > 8 {
        return (width, height, pixels);
    }

    let w = width as usize;
    let h = height as usize;
    let mut out = vec![0u8; pixels.len()];

    match orientation {
        2 => { // Flip Horizontal
            for y in 0..h {
                for x in 0..w {
                    let src = (y * w + x) * 4;
                    let dst = (y * w + (w - 1 - x)) * 4;
                    out[dst..dst + 4].copy_from_slice(&pixels[src..src + 4]);
                }
            }
            (width, height, out)
        }
        3 => { // Rotate 180
            for y in 0..h {
                for x in 0..w {
                    let src = (y * w + x) * 4;
                    let dst = ((h - 1 - y) * w + (w - 1 - x)) * 4;
                    out[dst..dst + 4].copy_from_slice(&pixels[src..src + 4]);
                }
            }
            (width, height, out)
        }
        4 => { // Flip Vertical
            for y in 0..h {
                let src = (y * w) * 4;
                let dst = ((h - 1 - y) * w) * 4;
                out[dst..dst + w * 4].copy_from_slice(&pixels[src..src + w * 4]);
            }
            (width, height, out)
        }
        5 => { // Transpose (Flip Horizontally & Rotate 90 CW)
            for y in 0..h {
                for x in 0..w {
                    let src = (y * w + x) * 4;
                    let dst = (x * h + y) * 4;
                    out[dst..dst + 4].copy_from_slice(&pixels[src..src + 4]);
                }
            }
            (height, width, out)
        }
        6 => { // Rotate 90 CW (Standard iPhone Portrait)
            for y in 0..h {
                for x in 0..w {
                    let src = (y * w + x) * 4;
                    let dst = (x * h + (h - 1 - y)) * 4;
                    out[dst..dst + 4].copy_from_slice(&pixels[src..src + 4]);
                }
            }
            (height, width, out)
        }
        7 => { // Transverse (Flip Horizontally & Rotate 90 CCW)
            for y in 0..h {
                for x in 0..w {
                    let src = (y * w + x) * 4;
                    let dst = ((w - 1 - x) * h + (h - 1 - y)) * 4;
                    out[dst..dst + 4].copy_from_slice(&pixels[src..src + 4]);
                }
            }
            (height, width, out)
        }
        8 => { // Rotate 90 CCW
            for y in 0..h {
                for x in 0..w {
                    let src = (y * w + x) * 4;
                    let dst = ((w - 1 - x) * h + y) * 4;
                    out[dst..dst + 4].copy_from_slice(&pixels[src..src + 4]);
                }
            }
            (height, width, out)
        }
        _ => (width, height, pixels),
    }
}

/// Spawns a dedicated background thread for heavy image decoding with versioning.
/// Returns a Sender to request (Path, ID) pairs, and a Receiver for decoded pixels.
pub fn spawn_image_loader(
    ctx: egui::Context,
    id_tracker: Arc<AtomicU64>, // Added tracker to monitor for stale requests
) -> (Sender<(PathBuf, u64)>, Receiver<Result<LoadedImage, LoadFailure>>) {
    spawn_image_loader_internal(ctx, id_tracker, true)
}

/// Spawns an ordered loader that processes queued requests in order.
/// Used by preloading so we can decode more than one target per generation.
pub fn spawn_image_loader_ordered(
    ctx: egui::Context,
    id_tracker: Arc<AtomicU64>,
) -> (Sender<(PathBuf, u64)>, Receiver<Result<LoadedImage, LoadFailure>>) {
    spawn_image_loader_internal(ctx, id_tracker, false)
}

fn spawn_image_loader_internal(
    ctx: egui::Context,
    id_tracker: Arc<AtomicU64>,
    keep_latest_only: bool,
) -> (Sender<(PathBuf, u64)>, Receiver<Result<LoadedImage, LoadFailure>>) {
    let (req_tx, req_rx) = channel::<(PathBuf, u64)>();
    let (res_tx, res_rx) = channel::<Result<LoadedImage, LoadFailure>>();

    std::thread::spawn(move || {
        while let Ok((mut path, mut req_id)) = req_rx.recv() {
            if keep_latest_only {
                // Foreground loading keeps only the newest request for responsiveness.
                while let Ok((newer_path, newer_id)) = req_rx.try_recv() {
                    path = newer_path;
                    req_id = newer_id;
                }
            }

            let res = decode_image_request(&path, req_id, &id_tracker);

            // 4. Send the result ONLY IF it is not stale
            match res {
                Ok(loaded_image) => {
                    let _ = res_tx.send(Ok(loaded_image));
                    ctx.request_repaint(); 
                }
                Err(e) if e.to_string() == "Stale Request" => {
                    // Silently drop stale results
                }
                Err(e) => {
                    let _ = res_tx.send(Err(LoadFailure {
                        request_id: req_id,
                        message: e.to_string(),
                    }));
                    ctx.request_repaint(); 
                }
            }
        }
    });

    (req_tx, res_rx)
}

fn decode_image_request(
    path: &PathBuf,
    req_id: u64,
    id_tracker: &Arc<AtomicU64>,
) -> Result<LoadedImage, Box<dyn std::error::Error + Send + Sync>> {
    // VERSION CHECK: Abort if a newer request was sent before we read the file.
    if id_tracker.load(Ordering::Acquire) != req_id {
        return Err("Stale Request".into());
    }
    let file_bytes = std::fs::read(path)?;

    // Determine format routing via magic bytes instead of extension.
    let ext_fallback = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();

    let format_str = if file_bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        "png"
    } else if file_bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        "jpg"
    } else if file_bytes.starts_with(b"RIFF") && file_bytes.len() >= 12 && &file_bytes[8..12] == b"WEBP" {
        "webp"
    } else if file_bytes.starts_with(b"GIF8") {
        "gif"
    } else if file_bytes.len() >= 12 && &file_bytes[4..12] == b"ftypavif" {
        "avif"
    } else if looks_like_heif(&file_bytes) {
        "heic"
    } else if file_bytes.starts_with(&[0xFF, 0x0A]) || file_bytes.starts_with(&[0x00, 0x00, 0x00, 0x0C, b'J', b'X', b'L']) {
        "jxl"
    } else {
        ext_fallback.as_str()
    };

    // VERSION CHECK: Abort before EXIF parsing if request is now stale.
    if id_tracker.load(Ordering::Acquire) != req_id {
        return Err("Stale Request".into());
    }

    // libheif already applies HEIF transforms (rotation/mirroring/crop),
    // so running EXIF rotation again can double-rotate some files.
    let exif_meta = if matches!(format_str, "heic" | "heif" | "hif") {
        ExifMetadata {
            orientation: 1,
            density: None,
        }
    } else {
        exif_metadata_from_container(&file_bytes)
    };
    let exif_orientation = exif_meta.orientation;
    let density = exif_meta
        .density
        .or_else(|| density_from_container(format_str, &file_bytes));

    // VERSION CHECK: Abort before the heavy decoding step.
    if id_tracker.load(Ordering::Acquire) != req_id {
        return Err("Stale Request".into());
    }

    // Decode bytes using the most optimal crate available.
    let (width, height, frames) = match format_str {
        "webp" => {
            let decoder = webp::Decoder::new(&file_bytes);
            let webp_img = decoder.decode().ok_or("Failed to decode WebP")?;

            let w = webp_img.width();
            let h = webp_img.height();
            let mut px = webp_img.to_vec();

            if px.len() == (w * h * 3) as usize {
                px = pad_rgb_to_rgba(&px);
            }

            (w, h, vec![ImageFrame { pixels: px, duration_ms: 0 }])
        }
        "avif" => {
            let dynamic_img = libavif_image::read(&file_bytes)
                .map_err(|e| format!("AVIF Decode Error: {}", e))?;
            let w = dynamic_img.width();
            let h = dynamic_img.height();
            let px = dynamic_img.into_rgba8().into_raw();

            (w, h, vec![ImageFrame { pixels: px, duration_ms: 0 }])
        }
        "heic" | "heif" | "hif" => {
            use libheif_rs::{ColorSpace, HeifContext, LibHeif, RgbChroma};

            let context = HeifContext::read_from_bytes(&file_bytes)
                .map_err(|e| format!("HEIF Context Error: {}", e))?;
            let handle = context
                .primary_image_handle()
                .map_err(|e| format!("HEIF Primary Image Error: {}", e))?;

            let has_alpha = handle.has_alpha_channel();
            let requested_space = if has_alpha {
                ColorSpace::Rgb(RgbChroma::Rgba)
            } else {
                ColorSpace::Rgb(RgbChroma::Rgb)
            };

            let image = LibHeif::new()
                .decode(&handle, requested_space, None)
                .map_err(|e| format!("HEIF Decode Error: {}", e))?;

            let plane = image
                .planes()
                .interleaved
                .ok_or("HEIF Decode Error: image is not interleaved")?;

            let bytes_per_pixel = (plane.storage_bits_per_pixel / 8) as usize;
            if bytes_per_pixel != 3 && bytes_per_pixel != 4 {
                return Err(format!("HEIF Decode Error: unsupported pixel layout ({} bpp)", plane.storage_bits_per_pixel).into());
            }

            let row_size = plane.width as usize * bytes_per_pixel;
            if row_size > plane.stride {
                return Err("HEIF Decode Error: row size exceeds stride".into());
            }

            let mut px = Vec::with_capacity((plane.width * plane.height * bytes_per_pixel as u32) as usize);
            for row in plane
                .data
                .chunks_exact(plane.stride)
                .take(plane.height as usize)
            {
                px.extend_from_slice(&row[..row_size]);
            }

            if bytes_per_pixel == 3 {
                px = pad_rgb_to_rgba(&px);
            }

            (
                plane.width,
                plane.height,
                vec![ImageFrame {
                    pixels: px,
                    duration_ms: 0,
                }],
            )
        }
        "jxl" => {
            let jxl_decoder = jxl_oxide::integration::JxlDecoder::new(std::io::Cursor::new(&file_bytes))
                .map_err(|e| format!("JXL Error: {}", e))?;
            let dynamic_img = image::DynamicImage::from_decoder(jxl_decoder)
                .map_err(|e| format!("JXL Dynamic Image Error: {}", e))?;

            let w = dynamic_img.width();
            let h = dynamic_img.height();
            let px = dynamic_img.into_rgba8().into_raw();

            (w, h, vec![ImageFrame { pixels: px, duration_ms: 0 }])
        }
        "png" => {
            use zune_jpeg::zune_core::bytestream::ZCursor;
            use zune_jpeg::zune_core::result::DecodingResult;
            use zune_png::PngDecoder;

            let cursor = ZCursor::new(&file_bytes);
            let mut decoder = PngDecoder::new(cursor);

            decoder.decode_headers().map_err(|e| format!("PNG Header Error: {:?}", e))?;
            let info = decoder.info().ok_or("Failed to get PNG info")?;
            let w = info.width as u32;
            let h = info.height as u32;

            let decoded_enum = decoder.decode().map_err(|e| format!("PNG Decode Error: {:?}", e))?;

            let mut px = match decoded_enum {
                DecodingResult::U8(data) => data,
                DecodingResult::U16(data) => data.into_iter().map(|v| (v >> 8) as u8).collect(),
                _ => return Err("Unsupported PNG bit depth".into()),
            };

            if px.len() == (w * h * 3) as usize {
                px = pad_rgb_to_rgba(&px);
            }

            (w, h, vec![ImageFrame { pixels: px, duration_ms: 0 }])
        }
        "jpg" | "jpeg" => {
            use zune_jpeg::zune_core::bytestream::ZCursor;
            use zune_jpeg::zune_core::colorspace::ColorSpace;
            use zune_jpeg::zune_core::options::DecoderOptions;

            let options = DecoderOptions::default().jpeg_set_out_colorspace(ColorSpace::RGBA);
            let cursor = ZCursor::new(&file_bytes);
            let mut decoder = zune_jpeg::JpegDecoder::new_with_options(cursor, options);

            decoder.decode_headers()?;
            let info = decoder.info().ok_or("Failed to read JPEG headers")?;
            let w = info.width as u32;
            let h = info.height as u32;
            let px = decoder.decode()?;

            (w, h, vec![ImageFrame { pixels: px, duration_ms: 0 }])
        }
        "gif" => {
            use image::AnimationDecoder;
            let decoder = image::codecs::gif::GifDecoder::new(std::io::Cursor::new(&file_bytes))
                .map_err(|e| format!("GIF Decoder Error: {}", e))?;

            let mut frames = Vec::new();
            let mut w = 0;
            let mut h = 0;

            for (i, frame_res) in decoder.into_frames().enumerate() {
                if id_tracker.load(Ordering::Acquire) != req_id {
                    return Err("Stale Request".into());
                }

                let frame = frame_res.map_err(|e| format!("GIF Frame Error: {}", e))?;
                let img = frame.buffer();

                if i == 0 {
                    w = img.width();
                    h = img.height();
                }

                let (num, den) = frame.delay().numer_denom_ms();
                let raw_duration_ms = if den > 0 && num > 0 { num / den } else { 100 };
                let duration_ms = raw_duration_ms.max(MIN_ANIM_FRAME_MS);

                frames.push(ImageFrame {
                    pixels: img.clone().into_raw(),
                    duration_ms,
                });
            }

            (w, h, frames)
        }
        _ => {
            let img = image::load_from_memory(&file_bytes)?;
            let w = img.width();
            let h = img.height();
            let px = img.to_rgba8().into_raw();

            (w, h, vec![ImageFrame { pixels: px, duration_ms: 0 }])
        }
    };

    if id_tracker.load(Ordering::Acquire) != req_id {
        return Err("Stale Request".into());
    }

    let mut oriented_frames = Vec::with_capacity(frames.len());
    let mut final_w = width;
    let mut final_h = height;

    for frame in frames {
        if id_tracker.load(Ordering::Acquire) != req_id {
            return Err("Stale Request".into());
        }

        let (nw, nh, npix) = apply_exif_orientation(frame.pixels, width, height, exif_orientation);
        final_w = nw;
        final_h = nh;
        oriented_frames.push(ImageFrame {
            pixels: npix,
            duration_ms: frame.duration_ms,
        });
    }

    Ok(LoadedImage {
        request_id: req_id,
        width: final_w,
        height: final_h,
        density,
        frames: oriented_frames,
    })
}

fn parse_exif_orientation(exif: &exif::Exif) -> Option<u32> {
    exif.get_field(exif::Tag::Orientation, exif::In::PRIMARY)
        .and_then(|field| field.value.get_uint(0))
        .map(|v| v as u32)
        .filter(|v| (1..=8).contains(v))
}

struct ExifMetadata {
    orientation: u32,
    density: Option<ImageDensity>,
}

fn exif_metadata_from_container(file_bytes: &[u8]) -> ExifMetadata {
    match exif::Reader::new().read_from_container(&mut std::io::Cursor::new(file_bytes)) {
        Ok(exif) => ExifMetadata {
            orientation: parse_exif_orientation(&exif).unwrap_or(1),
            density: parse_exif_density(&exif),
        },
        Err(_) => ExifMetadata {
            orientation: 1,
            density: None,
        },
    }
}

fn parse_exif_density(exif: &exif::Exif) -> Option<ImageDensity> {
    use exif::Tag;

    let unit = exif
        .get_field(Tag::ResolutionUnit, exif::In::PRIMARY)
        .and_then(|field| field.value.get_uint(0))
        .unwrap_or(2);

    let x = exif
        .get_field(Tag::XResolution, exif::In::PRIMARY)
        .and_then(|field| exif_value_to_f32(&field.value));
    let y = exif
        .get_field(Tag::YResolution, exif::In::PRIMARY)
        .and_then(|field| exif_value_to_f32(&field.value));

    let base = match (x, y) {
        (Some(px), Some(py)) => Some((px, py)),
        (Some(p), None) | (None, Some(p)) => Some((p, p)),
        (None, None) => None,
    }?;

    let (mut ppi_x, mut ppi_y) = base;
    match unit {
        2 => {} // inches
        3 => {
            // centimeters -> inches
            ppi_x *= 2.54;
            ppi_y *= 2.54;
        }
        _ => return None, // unknown/no unit: no reliable physical density
    }

    normalize_density(ppi_x, ppi_y, DensitySource::Exif)
}

fn exif_value_to_f32(value: &exif::Value) -> Option<f32> {
    match value {
        exif::Value::Rational(vals) => vals.first().and_then(|r| {
            if r.denom == 0 {
                None
            } else {
                Some(r.num as f32 / r.denom as f32)
            }
        }),
        exif::Value::SRational(vals) => vals.first().and_then(|r| {
            if r.denom == 0 {
                None
            } else {
                Some(r.num as f32 / r.denom as f32)
            }
        }),
        exif::Value::Short(vals) => vals.first().map(|v| *v as f32),
        exif::Value::Long(vals) => vals.first().map(|v| *v as f32),
        _ => None,
    }
}

fn normalize_density(ppi_x: f32, ppi_y: f32, source: DensitySource) -> Option<ImageDensity> {
    let finite = ppi_x.is_finite() && ppi_y.is_finite();
    let in_range = ppi_x > 1.0 && ppi_y > 1.0 && ppi_x < 20_000.0 && ppi_y < 20_000.0;
    if !finite || !in_range {
        return None;
    }

    Some(ImageDensity {
        ppi_x,
        ppi_y,
        source,
    })
}

fn density_from_container(format: &str, file_bytes: &[u8]) -> Option<ImageDensity> {
    match format {
        "png" => parse_png_phys_density(file_bytes),
        "jpg" | "jpeg" => parse_jpeg_jfif_density(file_bytes),
        _ => None,
    }
}

fn parse_png_phys_density(file_bytes: &[u8]) -> Option<ImageDensity> {
    const PNG_SIG: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
    if file_bytes.len() < 8 || &file_bytes[0..8] != PNG_SIG {
        return None;
    }

    let mut offset = 8usize;
    while offset + 12 <= file_bytes.len() {
        let len = u32::from_be_bytes([
            file_bytes[offset],
            file_bytes[offset + 1],
            file_bytes[offset + 2],
            file_bytes[offset + 3],
        ]) as usize;
        let chunk_start = offset + 8;
        let chunk_end = chunk_start.saturating_add(len);
        if chunk_end + 4 > file_bytes.len() {
            break;
        }

        let chunk_type = &file_bytes[offset + 4..offset + 8];
        if chunk_type == b"pHYs" && len >= 9 {
            let data = &file_bytes[chunk_start..chunk_end];
            let x_ppu = u32::from_be_bytes([data[0], data[1], data[2], data[3]]) as f32;
            let y_ppu = u32::from_be_bytes([data[4], data[5], data[6], data[7]]) as f32;
            let unit = data[8];

            if unit == 1 {
                // pixels per meter -> pixels per inch
                let ppi_x = x_ppu * 0.0254;
                let ppi_y = y_ppu * 0.0254;
                return normalize_density(ppi_x, ppi_y, DensitySource::Container);
            }
            return None;
        }

        offset = chunk_end + 4;
    }

    None
}

fn parse_jpeg_jfif_density(file_bytes: &[u8]) -> Option<ImageDensity> {
    if file_bytes.len() < 4 || file_bytes[0] != 0xFF || file_bytes[1] != 0xD8 {
        return None;
    }

    let mut i = 2usize;
    while i + 4 <= file_bytes.len() {
        if file_bytes[i] != 0xFF {
            i += 1;
            continue;
        }

        while i < file_bytes.len() && file_bytes[i] == 0xFF {
            i += 1;
        }
        if i >= file_bytes.len() {
            break;
        }

        let marker = file_bytes[i];
        i += 1;

        if marker == 0xD9 || marker == 0xDA {
            break;
        }

        if i + 2 > file_bytes.len() {
            break;
        }
        let seg_len = u16::from_be_bytes([file_bytes[i], file_bytes[i + 1]]) as usize;
        i += 2;
        if seg_len < 2 || i + (seg_len - 2) > file_bytes.len() {
            break;
        }

        if marker == 0xE0 && seg_len >= 16 {
            let segment = &file_bytes[i..i + (seg_len - 2)];
            if segment.len() >= 14 && &segment[0..5] == b"JFIF\0" {
                let unit = segment[7];
                let x_density = u16::from_be_bytes([segment[8], segment[9]]) as f32;
                let y_density = u16::from_be_bytes([segment[10], segment[11]]) as f32;

                let (ppi_x, ppi_y) = match unit {
                    1 => (x_density, y_density),
                    2 => (x_density * 2.54, y_density * 2.54),
                    _ => return None,
                };

                return normalize_density(ppi_x, ppi_y, DensitySource::Container);
            }
        }

        i += seg_len - 2;
    }

    None
}

