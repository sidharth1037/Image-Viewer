use eframe::egui; 
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};

pub struct ImageFrame {
    pub pixels: Vec<u8>,
    pub duration_ms: u32,
}

pub struct LoadedImage {
    pub filename: String, // <-- NEW: Identify the image
    pub width: u32,
    pub height: u32,
    pub frames: Vec<ImageFrame>, 
}

fn pad_rgb_to_rgba(rgb_pixels: &[u8], width: u32, height: u32) -> Vec<u8> {
    let mut rgba_pixels = Vec::with_capacity((width * height * 4) as usize);
    for chunk in rgb_pixels.chunks_exact(3) {
        rgba_pixels.extend_from_slice(&[chunk[0], chunk[1], chunk[2], 255]);
    }
    rgba_pixels
}

// Notice the Err type is now (String, String) to pass the filename back on failure
pub fn spawn_image_loader(ctx: egui::Context) -> (Sender<PathBuf>, Receiver<Result<LoadedImage, (String, String)>>) {
    let (req_tx, req_rx) = channel::<PathBuf>();
    let (res_tx, res_rx) = channel::<Result<LoadedImage, (String, String)>>();

    std::thread::spawn(move || {
        // Notice we made `path` mutable here so we can update it in the drain loop
        while let Ok(mut path) = req_rx.recv() {
            
            // --- 1. THE QUEUE DRAIN ---
            // If the user mashed the arrow keys while we were sleeping or decoding, 
            // grab the absolute newest path and throw away all the intermediate ones!
            while let Ok(newer_path) = req_rx.try_recv() {
                path = newer_path;
            }

            // Extract filename to pass back to the UI
            let filename = path.file_name().unwrap_or_default().to_string_lossy().into_owned();
            let err_filename = filename.clone(); // Kept for the error block

            let res = (|| -> Result<LoadedImage, Box<dyn std::error::Error + Send + Sync>> {
                
                println!("--- Loading Image: {} ---", filename);
                let total_start = std::time::Instant::now();
                
                // 1. Instantly load file bytes into RAM
                let disk_start = std::time::Instant::now();
                let file_bytes = std::fs::read(&path)?;
                println!("[Disk] Read into RAM took: {:?}", disk_start.elapsed());
                
                // 2. Extract the file extension
                let ext = path.extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_lowercase();

                // 3. Route to the fastest decoder based on extension
                let (width, height, frames) = match ext.as_str() {
                    
                    "webp" => {
                        // --- NATIVE C-LIBRARY FOR WEBP ---
                        let decode_start = std::time::Instant::now();
                        let decoder = webp::Decoder::new(&file_bytes);
                        let webp_img = decoder.decode().ok_or("Failed to decode WebP")?;
                        println!("[WebP] Native C-Library Decoding took: {:?}", decode_start.elapsed());
                        
                        let w = webp_img.width();
                        let h = webp_img.height();
                        let mut px = webp_img.to_vec(); 
                        
                        // Check if padding to RGBA is necessary
                        if px.len() == (w * h * 3) as usize {
                            let pad_start = std::time::Instant::now();
                            px = pad_rgb_to_rgba(&px, w, h);
                            println!("[WebP] RGB to RGBA padding took: {:?}", pad_start.elapsed());
                        } else {
                            println!("[WebP] Image already has alpha channel (no padding needed).");
                        }
                        
                        // Wrap in a single ImageFrame
                        (w, h, vec![ImageFrame { pixels: px, duration_ms: 0 }])
                    }

                    "avif" => {
                        // --- NATIVE AVIF DECODER (using dav1d via libavif) ---
                        let decode_start = std::time::Instant::now();
                        
                        let dynamic_img = libavif_image::read(&file_bytes)
                            .map_err(|e| format!("AVIF Decode Error: {}", e))?;
                        
                        let w = dynamic_img.width();
                        let h = dynamic_img.height();
                        
                        let px = dynamic_img.into_rgba8().into_raw();
                        println!("[AVIF] Native libavif Decoding took: {:?}", decode_start.elapsed());
                        
                        (w, h, vec![ImageFrame { pixels: px, duration_ms: 0 }])
                    }

                    "jxl" => {
                        // --- PURE RUST JPEG XL (jxl-oxide) ---
                        let decode_start = std::time::Instant::now();
                        
                        let jxl_decoder = jxl_oxide::integration::JxlDecoder::new(std::io::Cursor::new(&file_bytes))
                            .map_err(|e| format!("JXL Error: {}", e))?;
                            
                        let dynamic_img = image::DynamicImage::from_decoder(jxl_decoder)
                            .map_err(|e| format!("JXL Dynamic Image Error: {}", e))?;
                            
                        let w = dynamic_img.width();
                        let h = dynamic_img.height();
                        
                        let px = dynamic_img.into_rgba8().into_raw();
                        println!("[JXL] Pure Rust jxl-oxide Decoding took: {:?}", decode_start.elapsed());
                        
                        (w, h, vec![ImageFrame { pixels: px, duration_ms: 0 }])
                    }

                    "png" => {
                        // --- FAST SIMD ACCELERATION FOR PNG ---
                        let decode_start = std::time::Instant::now();
                        use zune_png::PngDecoder;
                        use zune_jpeg::zune_core::bytestream::ZCursor;
                        use zune_jpeg::zune_core::result::DecodingResult;
                        
                        let cursor = ZCursor::new(&file_bytes);
                        let mut decoder = PngDecoder::new(cursor);
                        
                        decoder.decode_headers().map_err(|e| format!("PNG Header Error: {:?}", e))?;
                        let info = decoder.info().ok_or("Failed to get PNG info")?;
                        
                        let w = info.width as u32;
                        let h = info.height as u32;
                        
                        let decoded_enum = decoder.decode().map_err(|e| format!("PNG Decode Error: {:?}", e))?;
                        println!("[PNG] Zune SIMD Decoding took: {:?}", decode_start.elapsed());

                        let mut px = match decoded_enum {
                            DecodingResult::U8(data) => data,
                            DecodingResult::U16(data) => {
                                data.into_iter().map(|v| (v >> 8) as u8).collect()
                            },
                            _ => return Err("Unsupported PNG bit depth (Not 8 or 16-bit)".into()),
                        };

                        if px.len() == (w * h * 3) as usize {
                            let pad_start = std::time::Instant::now();
                            px = pad_rgb_to_rgba(&px[..], w, h);
                            println!("[PNG] RGB to RGBA padding took: {:?}", pad_start.elapsed());
                        }

                        (w, h, vec![ImageFrame { pixels: px, duration_ms: 0 }])
                    }
                    
                    "jpg" | "jpeg" => {
                        // --- FAST SIMD ACCELERATION FOR JPEG ---
                        let decode_start = std::time::Instant::now();
                        use zune_jpeg::zune_core::options::DecoderOptions;
                        use zune_jpeg::zune_core::colorspace::ColorSpace;
                        use zune_jpeg::zune_core::bytestream::ZCursor;

                        let options = DecoderOptions::default()
                            .jpeg_set_out_colorspace(ColorSpace::RGBA);
                            
                        let cursor = ZCursor::new(&file_bytes);
                        
                        let mut decoder = zune_jpeg::JpegDecoder::new_with_options(cursor, options);
                        decoder.decode_headers()?;
                        let info = decoder.info().ok_or("Failed to read JPEG headers")?;
                        
                        let w = info.width as u32;
                        let h = info.height as u32;
                        
                        let px = decoder.decode()?; 
                        println!("[JPEG] Zune SIMD Decoding (Direct to RGBA) took: {:?}", decode_start.elapsed());
                        
                        (w, h, vec![ImageFrame { pixels: px, duration_ms: 0 }])
                    }
                    
                    "gif" => {
                        // --- ANIMATED GIF DECODING ---
                        let decode_start = std::time::Instant::now();
                        use image::AnimationDecoder;
                        
                        let decoder = image::codecs::gif::GifDecoder::new(std::io::Cursor::new(&file_bytes))
                            .map_err(|e| format!("GIF Decoder Error: {}", e))?;
                            
                        let frames_iter = decoder.into_frames();
                        let mut frames = Vec::new();
                        let mut w = 0;
                        let mut h = 0;
                        
                        for (i, frame_res) in frames_iter.enumerate() {
                            let frame = frame_res.map_err(|e| format!("GIF Frame Error: {}", e))?;
                            let img = frame.buffer();
                            
                            if i == 0 {
                                w = img.width();
                                h = img.height();
                            }
                            
                            let (num, den) = frame.delay().numer_denom_ms();
                            let duration = if den > 0 { num / den } else { 100 };
                            let duration_ms = if duration > 0 { duration } else { 100 };
                            
                            frames.push(ImageFrame {
                                pixels: img.clone().into_raw(),
                                duration_ms,
                            });
                        }
                        
                        println!("[GIF] Animated Decoding ({} frames) took: {:?}", frames.len(), decode_start.elapsed());
                        
                        (w, h, frames) 
                    }
                    
                    _ => {
                        // --- FALLBACK FOR TIFF, BMP, ICO, ETC ---
                        let decode_start = std::time::Instant::now();
                        let img = image::load_from_memory(&file_bytes)?;
                        println!("[Fallback] Standard Rust Decoding took: {:?}", decode_start.elapsed());
                        
                        let w = img.width();
                        let h = img.height();
                        let px = img.to_rgba8().into_raw();
                        
                        (w, h, vec![ImageFrame { pixels: px, duration_ms: 0 }])
                    }
                };

                println!("[Total] Background Processing Time: {:?}", total_start.elapsed());
                println!("------------------------------------------------");

                // --- NEW: Package the filename into the struct here ---
                Ok(LoadedImage { filename, width, height, frames })
            })();

            // 4. Send the result and wake up the UI thread
            match res {
                Ok(loaded_image) => {
                    let _ = res_tx.send(Ok(loaded_image));
                    ctx.request_repaint(); 
                }
                Err(e) => {
                    // --- NEW: Send the filename back alongside the error string ---
                    let _ = res_tx.send(Err((err_filename, e.to_string())));
                    ctx.request_repaint(); 
                }
            }
        }
    });

    (req_tx, res_rx)
}