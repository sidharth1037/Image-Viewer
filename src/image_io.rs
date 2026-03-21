use eframe::egui;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};

/// Represents a single frame of an image, ready for GPU upload.
pub struct ImageFrame {
    pub pixels: Vec<u8>,
    pub duration_ms: u32,
}

/// The final payload sent from the background decoding thread to the UI thread.
pub struct LoadedImage {
    pub filename: String,
    pub width: u32,
    pub height: u32,
    pub frames: Vec<ImageFrame>, // Holds 1 frame for static images, many for GIFs
}

/// Helper function to rapidly convert 3-channel RGB data into 4-channel RGBA data.
/// eframe/egui strictly requires an Alpha channel for texture rendering.
fn pad_rgb_to_rgba(rgb_pixels: &[u8]) -> Vec<u8> {
    let mut rgba_pixels = Vec::with_capacity((rgb_pixels.len() / 3) * 4);
    for chunk in rgb_pixels.chunks_exact(3) {
        rgba_pixels.extend_from_slice(&[chunk[0], chunk[1], chunk[2], 255]);
    }
    rgba_pixels
}

/// Spawns a dedicated background thread for heavy image decoding.
/// Returns a Sender to request paths, and a Receiver to get the decoded pixels.
pub fn spawn_image_loader(
    ctx: egui::Context,
) -> (Sender<PathBuf>, Receiver<Result<LoadedImage, (String, String)>>) {
    let (req_tx, req_rx) = channel::<PathBuf>();
    let (res_tx, res_rx) = channel::<Result<LoadedImage, (String, String)>>();

    std::thread::spawn(move || {
        while let Ok(mut path) = req_rx.recv() {
            
            // --- QUEUE DRAIN OPTIMIZATION ---
            // If the user rapidly skips through images (e.g., holding arrow keys),
            // this loop grabs the absolute newest request and discards the stale ones,
            // preventing the background thread from building up a massive backlog.
            while let Ok(newer_path) = req_rx.try_recv() {
                path = newer_path;
            }

            let filename = path.file_name().unwrap_or_default().to_string_lossy().into_owned();
            let err_filename = filename.clone(); 

            let res = (|| -> Result<LoadedImage, Box<dyn std::error::Error + Send + Sync>> {
                
                // 1. Instantly read file bytes into RAM
                let file_bytes = std::fs::read(&path)?;
                
                // 2. Determine format routing via MAGIC BYTES instead of extension
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
                } else if file_bytes.starts_with(&[0xFF, 0x0A]) || file_bytes.starts_with(&[0x00, 0x00, 0x00, 0x0C, b'J', b'X', b'L']) {
                    "jxl"
                } else {
                    ext_fallback.as_str() // Fallback to extension if magic bytes are unknown
                };

                // 3. Decode bytes using the most optimal crate available
                let (width, height, frames) = match format_str {
                    
                    "webp" => {
                        // Native C-Library Binding (webp crate)
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
                        // dav1d C-Library via libavif
                        let dynamic_img = libavif_image::read(&file_bytes)
                            .map_err(|e| format!("AVIF Decode Error: {}", e))?;
                        
                        let w = dynamic_img.width();
                        let h = dynamic_img.height();
                        let px = dynamic_img.into_rgba8().into_raw();
                        
                        (w, h, vec![ImageFrame { pixels: px, duration_ms: 0 }])
                    }

                    "jxl" => {
                        // Pure Rust jxl-oxide decoder
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
                        // SIMD Accelerated Pure Rust PNG Decoder
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
                        
                        // Handle 8-bit and 16-bit depths
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
                        // SIMD Accelerated Pure Rust JPEG Decoder
                        use zune_jpeg::zune_core::bytestream::ZCursor;
                        use zune_jpeg::zune_core::colorspace::ColorSpace;
                        use zune_jpeg::zune_core::options::DecoderOptions;

                        // Force decoder to output RGBA directly to skip manual padding
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
                        // Multi-frame Animated GIF Decoder
                        use image::AnimationDecoder;
                        let decoder = image::codecs::gif::GifDecoder::new(std::io::Cursor::new(&file_bytes))
                            .map_err(|e| format!("GIF Decoder Error: {}", e))?;
                            
                        let mut frames = Vec::new();
                        let mut w = 0;
                        let mut h = 0;
                        
                        // Automatically handles complex disposal methods and delta blending
                        for (i, frame_res) in decoder.into_frames().enumerate() {
                            let frame = frame_res.map_err(|e| format!("GIF Frame Error: {}", e))?;
                            let img = frame.buffer();
                            
                            if i == 0 {
                                w = img.width();
                                h = img.height();
                            }
                            
                            let (num, den) = frame.delay().numer_denom_ms();
                            // Fallback to 100ms (10fps) if the GIF has corrupted timing data
                            let duration_ms = if den > 0 && num > 0 { num / den } else { 100 };
                            
                            frames.push(ImageFrame {
                                pixels: img.clone().into_raw(),
                                duration_ms,
                            });
                        }
                        
                        (w, h, frames) 
                    }
                    
                    _ => {
                        // Standard Rust 'image' crate fallback (Handles BMP, TIFF, ICO, etc.)
                        let img = image::load_from_memory(&file_bytes)?;
                        let w = img.width();
                        let h = img.height();
                        let px = img.to_rgba8().into_raw();
                        
                        (w, h, vec![ImageFrame { pixels: px, duration_ms: 0 }])
                    }
                };

                Ok(LoadedImage { filename, width, height, frames })
            })();

            // 4. Send the result and wake up the UI thread for an immediate frame update
            match res {
                Ok(loaded_image) => {
                    let _ = res_tx.send(Ok(loaded_image));
                    ctx.request_repaint(); 
                }
                Err(e) => {
                    let _ = res_tx.send(Err((err_filename, e.to_string())));
                    ctx.request_repaint(); 
                }
            }
        }
    });

    (req_tx, res_rx)
}