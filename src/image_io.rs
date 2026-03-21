use eframe::egui; 
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};

pub struct LoadedImage {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

// Helper function to quickly pad 3-channel RGB to 4-channel RGBA
fn pad_rgb_to_rgba(rgb_pixels: &[u8], width: u32, height: u32) -> Vec<u8> {
    let mut rgba_pixels = Vec::with_capacity((width * height * 4) as usize);
    for chunk in rgb_pixels.chunks_exact(3) {
        rgba_pixels.extend_from_slice(&[chunk[0], chunk[1], chunk[2], 255]);
    }
    rgba_pixels
}

pub fn spawn_image_loader(ctx: egui::Context) -> (Sender<PathBuf>, Receiver<Result<LoadedImage, String>>) {
    let (req_tx, req_rx) = channel::<PathBuf>();
    let (res_tx, res_rx) = channel::<Result<LoadedImage, String>>();

    std::thread::spawn(move || {
        while let Ok(path) = req_rx.recv() {
            let res = (|| -> Result<LoadedImage, Box<dyn std::error::Error + Send + Sync>> {
                
                println!("--- Loading Image: {:?} ---", path.file_name().unwrap_or_default());
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
                let (width, height, pixels) = match ext.as_str() {
                    
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
                        (w, h, px)
                    }

                    "avif" => {
                        // --- NATIVE AVIF DECODER (using dav1d via libavif) ---
                        let decode_start = std::time::Instant::now();
                        
                        let dynamic_img = libavif_image::read(&file_bytes)
                            .map_err(|e| format!("AVIF Decode Error: {}", e))?;
                        
                        let w = dynamic_img.width();
                        let h = dynamic_img.height();
                        
                        // Safely extract the raw RGBA pixels
                        let px = dynamic_img.into_rgba8().into_raw();
                        
                        println!("[AVIF] Native libavif Decoding took: {:?}", decode_start.elapsed());
                        (w, h, px)
                    }

                    "jxl" => {
                        // --- PURE RUST JPEG XL (jxl-oxide) ---
                        let decode_start = std::time::Instant::now();
                        
                        let jxl_decoder = jxl_oxide::integration::JxlDecoder::new(std::io::Cursor::new(&file_bytes))
                            .map_err(|e| format!("JXL Error: {}", e))?;
                            
                        // Bridge the JXL decoder with the standard image crate
                        let dynamic_img = image::DynamicImage::from_decoder(jxl_decoder)
                            .map_err(|e| format!("JXL Dynamic Image Error: {}", e))?;
                            
                        let w = dynamic_img.width();
                        let h = dynamic_img.height();
                        
                        // Safely extract the raw RGBA pixels
                        let px = dynamic_img.into_rgba8().into_raw();
                        
                        println!("[JXL] Pure Rust jxl-oxide Decoding took: {:?}", decode_start.elapsed());
                        (w, h, px)
                    }

                    "png" => {
                        // --- FAST SIMD ACCELERATION FOR PNG (zune-png 0.5+) ---
                        let decode_start = std::time::Instant::now();
                        use zune_png::PngDecoder;
                        use zune_jpeg::zune_core::bytestream::ZCursor;
                        use zune_jpeg::zune_core::result::DecodingResult;
                        
                        // 1. Wrap the bytes in a ZCursor
                        let cursor = ZCursor::new(&file_bytes);
                        let mut decoder = PngDecoder::new(cursor);
                        
                        // 2. Decode headers and get info
                        decoder.decode_headers().map_err(|e| format!("PNG Header Error: {:?}", e))?;
                        let info = decoder.info().ok_or("Failed to get PNG info")?;
                        
                        let w = info.width as u32;
                        let h = info.height as u32;
                        
                        // 3. Decode returns an enum (DecodingResult) to handle 8-bit vs 16-bit
                        let decoded_enum = decoder.decode().map_err(|e| format!("PNG Decode Error: {:?}", e))?;
                        println!("[PNG] Zune SIMD Decoding took: {:?}", decode_start.elapsed());

                        // 4. Safely extract the Vec<u8> based on the bit depth
                        let mut px = match decoded_enum {
                            DecodingResult::U8(data) => data,
                            DecodingResult::U16(data) => {
                                // If it's a 16-bit PNG, quickly downsample to 8-bit for egui
                                data.into_iter().map(|v| (v >> 8) as u8).collect()
                            },
                            _ => return Err("Unsupported PNG bit depth (Not 8 or 16-bit)".into()),
                        };

                        // 5. Pad RGB (3 channels) to RGBA (4 channels) if necessary
                        if px.len() == (w * h * 3) as usize {
                            let pad_start = std::time::Instant::now();
                            px = pad_rgb_to_rgba(&px[..], w, h);
                            println!("[PNG] RGB to RGBA padding took: {:?}", pad_start.elapsed());
                        }

                        (w, h, px)
                    }
                    
                    "jpg" | "jpeg" => {
                        // --- FAST SIMD ACCELERATION FOR JPEG ---
                        let decode_start = std::time::Instant::now();
                        use zune_jpeg::zune_core::options::DecoderOptions;
                        use zune_jpeg::zune_core::colorspace::ColorSpace;
                        use zune_jpeg::zune_core::bytestream::ZCursor;

                        // Tell the decoder to output RGBA directly, eliminating manual padding
                        let options = DecoderOptions::default()
                            .jpeg_set_out_colorspace(ColorSpace::RGBA);
                            
                        let cursor = ZCursor::new(&file_bytes);
                        
                        let mut decoder = zune_jpeg::JpegDecoder::new_with_options(cursor, options);
                        decoder.decode_headers()?;
                        let info = decoder.info().ok_or("Failed to read JPEG headers")?;
                        
                        let w = info.width as u32;
                        let h = info.height as u32;
                        
                        // Natively returns a Vec<u8> in RGBA format
                        let px = decoder.decode()?; 
                        println!("[JPEG] Zune SIMD Decoding (Direct to RGBA) took: {:?}", decode_start.elapsed());
                        
                        (w, h, px)
                    }
                    
                    _ => {
                        // --- FALLBACK FOR GIF, BMP, TIFF, ICO, ETC ---
                        let decode_start = std::time::Instant::now();
                        let img = image::load_from_memory(&file_bytes)?;
                        println!("[Fallback] Standard Rust Decoding took: {:?}", decode_start.elapsed());
                        
                        let color_start = std::time::Instant::now();
                        let w = img.width();
                        let h = img.height();
                        let px = img.to_rgba8().into_raw();
                        println!("[Fallback] Color Conversion (to RGBA) took: {:?}", color_start.elapsed());
                        
                        (w, h, px)
                    }
                };

                println!("[Total] Background Processing Time: {:?}", total_start.elapsed());
                println!("------------------------------------------------");

                Ok(LoadedImage { width, height, pixels })
            })();

            // 4. Send the result and wake up the UI thread
            match res {
                Ok(loaded_image) => {
                    let _ = res_tx.send(Ok(loaded_image));
                    ctx.request_repaint(); 
                }
                Err(e) => {
                    let _ = res_tx.send(Err(e.to_string()));
                    ctx.request_repaint(); 
                }
            }
        }
    });

    (req_tx, res_rx)
}