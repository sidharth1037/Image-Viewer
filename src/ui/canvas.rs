use eframe::egui;
use crate::state::ViewerState;

const DOUBLE_CLICK_ZOOM_LEVEL: f32 = 2.5; // 250%

pub fn render(ctx: &egui::Context, ui: &mut egui::Ui, state: &mut ViewerState) {
    let rect = ui.max_rect();
    ui.painter().rect_filled(rect, 0.0, ui.visuals().window_fill());

    if !state.frames.is_empty() {
        let current_time = ctx.input(|i| i.time);
        
        // --- ANIMATION PLAYER LOGIC ---
        if state.frames.len() > 1 {
            if let Some(last_time) = state.last_frame_time {
                let duration = state.frame_durations[state.current_frame];
                if current_time - last_time >= duration {
                    state.current_frame = (state.current_frame + 1) % state.frames.len();
                    state.last_frame_time = Some(current_time);
                }
                let next_frame_in = (duration - (current_time - last_time)).max(0.0);
                ctx.request_repaint_after(std::time::Duration::from_secs_f64(next_frame_in));
            } else {
                state.last_frame_time = Some(current_time);
                ctx.request_repaint();
            }
        }

        let texture = &state.frames[state.current_frame];
        let canvas_size = ui.available_size();
        let (response, painter) = ui.allocate_painter(canvas_size, egui::Sense::click_and_drag());
        let image_size = texture.size_vec2();

        let scale_w = canvas_size.x / image_size.x;
        let scale_h = canvas_size.y / image_size.y;
        let fit_scale = scale_w.min(scale_h);

        if state.auto_fit {
            state.scale = fit_scale;
            state.pan = egui::Vec2::ZERO;
        }

        // --- CONTEXT-AWARE DOUBLE CLICK ---
        if response.double_clicked() {
            state.auto_fit = false;
            let is_fitted = (state.scale - fit_scale).abs() < 0.001;

            if is_fitted {
                let zoom_factor = DOUBLE_CLICK_ZOOM_LEVEL;
                state.target_scale = Some(fit_scale * zoom_factor);
                if let Some(pointer_pos) = response.hover_pos() {
                    let canvas_center = response.rect.center();
                    let pointer_offset = pointer_pos - canvas_center;
                    state.target_pan = Some(-pointer_offset * (zoom_factor - 1.0));
                }
            } else {
                state.target_scale = Some(fit_scale);
                state.target_pan = Some(egui::Vec2::ZERO);
            }
            state.reset_start_time = Some(ui.input(|i| i.time));
        }

        // --- STATELESS ANIMATION ---
        if let Some(start_time) = state.reset_start_time {
            let current_time = ui.input(|i| i.time);
            let elapsed = (current_time - start_time) as f32;
            let t = (elapsed / 0.35).clamp(0.0, 1.0);

            if let (Some(t_scale), Some(t_pan)) = (state.target_scale, state.target_pan) {
                let lerp_factor = 0.25; 
                state.scale += (t_scale - state.scale) * lerp_factor;
                state.pan += (t_pan - state.pan) * lerp_factor;

                if t >= 1.0 || ((t_scale - state.scale).abs() < 0.001 && (t_pan - state.pan).length() < 0.1) {
                    state.scale = t_scale;
                    state.pan = t_pan;
                    state.reset_start_time = None;
                    state.target_scale = None;
                    state.target_pan = None;

                    if (t_scale - fit_scale).abs() < 0.001 {
                        state.auto_fit = true;
                    }
                }
            }
            ctx.request_repaint();
        } else {
            // Handle Zoom & Pan Inputs
            if response.hovered() {
                let scroll = ctx.input(|i| i.smooth_scroll_delta.y); 
                if scroll != 0.0 {
                    if let Some(pointer_pos) = response.hover_pos() {
                        state.auto_fit = false;
                        let zoom_multiplier = (scroll * 0.005).exp();
                        let old_scale = state.scale;
                        let new_scale = (old_scale * zoom_multiplier).max(fit_scale);

                        let canvas_center = response.rect.center();
                        let pointer_offset = pointer_pos - canvas_center;
                        let scale_ratio = new_scale / old_scale;
                        
                        state.pan -= (pointer_offset - state.pan) * (scale_ratio - 1.0);
                        state.scale = new_scale;
                    }
                }
            }

            let is_zoomed_in = state.scale > fit_scale * 1.0001;

            if is_zoomed_in {
                if response.dragged_by(egui::PointerButton::Primary) {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                    state.auto_fit = false;
                    state.pan += response.drag_delta();
                }
                
                let scaled_size = image_size * state.scale;
                let max_pan_x = ((scaled_size.x - canvas_size.x) / 2.0).max(0.0);
                let max_pan_y = ((scaled_size.y - canvas_size.y) / 2.0).max(0.0);
                
                state.pan.x = state.pan.x.clamp(-max_pan_x, max_pan_x);
                state.pan.y = state.pan.y.clamp(-max_pan_y, max_pan_y);
            } else {
                state.scale = fit_scale;
                state.auto_fit = true;
                state.pan = egui::Vec2::ZERO;
            }
        }

        // Draw the Image
        let scaled_size = image_size * state.scale;
        let center_offset = (canvas_size - scaled_size) / 2.0;
        let image_top_left = response.rect.min + center_offset + state.pan;
        let draw_rect = egui::Rect::from_min_size(image_top_left, scaled_size);
        let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
        
        painter.image(texture.id(), draw_rect, uv, egui::Color32::WHITE);
    } else {
        ui.centered_and_justified(|ui| {
            if let Some(err) = &state.load_error {
                ui.label(egui::RichText::new(err).color(ui.visuals().error_fg_color));
            } else if state.current_file_name.is_empty() {
                ui.label("No image loaded.\nDrag and drop an image here.");
            } else {
                ui.spinner();
            }
        });
    }
}