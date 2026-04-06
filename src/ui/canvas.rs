use eframe::egui;
use crate::state::ViewerState;

const DOUBLE_CLICK_ZOOM_LEVEL: f32 = 2.5;

pub fn render(ctx: &egui::Context, ui: &mut egui::Ui, state: &mut ViewerState, loop_playlist: bool) -> Option<i32> {
    let mut nav_action = None;

    // Allocate a persistent interaction area for the entire canvas.
    // This ensures inputs are captured consistently even during image transitions.
    let canvas_size = ui.available_size();
    let (response, painter) = ui.allocate_painter(canvas_size, egui::Sense::click_and_drag());
    painter.rect_filled(response.rect, 0.0, ui.visuals().window_fill());

    let mut fit_scale = 1.0;
    let mut image_size = egui::Vec2::ZERO;

    if !state.frames.is_empty() {
        let texture = &state.frames[state.current_frame];
        image_size = texture.size_vec2();
        
        let scale_w = canvas_size.x / image_size.x;
        let scale_h = canvas_size.y / image_size.y;
        fit_scale = scale_w.min(scale_h).min(1.0);

        // When auto_fit is enabled, the image is locked to the window dimensions.
        if state.auto_fit {
            state.scale = fit_scale;
            state.pan = egui::Vec2::ZERO;
        }
    }

    // Only track pointer while the canvas interaction region actually owns it.
    // This prevents overlay UI (top/bottom bars) from leaking clicks into navigation.
    let pointer_pos = response.interact_pointer_pos().or(response.hover_pos());
    let left_zone_bound = response.rect.min.x + canvas_size.x * 0.08;
    let right_zone_bound = response.rect.max.x - canvas_size.x * 0.08;
    let pointer_in_canvas = response.contains_pointer();
    
    let in_left_zone = pointer_in_canvas && pointer_pos.map_or(false, |p| p.x < left_zone_bound);
    let in_right_zone = pointer_in_canvas && pointer_pos.map_or(false, |p| p.x > right_zone_bound);
    let in_nav_zone = in_left_zone || in_right_zone;

    // Keep edge affordance consistent: always show hand cursor in navigation zones.
    if in_nav_zone {
        ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    let is_zoomed_in = state.scale > fit_scale * 1.0001;

    // Auto-Fit logic for window resizing/maximizing:
    // If the window grows larger than the current manual zoom level, we re-engage auto_fit.
    if !state.frames.is_empty() && !is_zoomed_in && !state.auto_fit && state.reset_start_time.is_none() {
        state.auto_fit = true;
        state.scale = fit_scale;
        state.pan = egui::Vec2::ZERO;
    }

    // Determine navigation availability based on playlist position
    let playlist_len = state.active_playlist.len();
    let current_idx = state.current_index;
    let has_prev = playlist_len > 1 && (loop_playlist || current_idx > 0);
    let has_next = playlist_len > 1 && (loop_playlist || current_idx + 1 < playlist_len);

    // Navigation Input: Handles clicks and double-clicks within the 8% edge zones.
    // Double clicks in these zones will navigate twice rather than triggering zoom.
    if state.reset_start_time.is_none() {
        if response.clicked() || response.double_clicked() {
            if in_left_zone && has_prev {
                nav_action = Some(-1);
            } else if in_right_zone && has_next {
                nav_action = Some(1);
            }
        }
    }

    // Zoom Input: Handles double-click to toggle between fit-to-screen and 250% zoom.
    // This is disabled in navigation zones when zoomed out to prevent accidental scale changes.
    if response.double_clicked() && (!in_nav_zone || is_zoomed_in) && !state.frames.is_empty() {
        state.auto_fit = false;
        let is_fitted = (state.scale - fit_scale).abs() < 0.001;

        if is_fitted {
            let zoom_factor = DOUBLE_CLICK_ZOOM_LEVEL;
            let target_scale = fit_scale * zoom_factor;
            state.target_scale = Some(target_scale);
            
            if let Some(pos) = pointer_pos {
                let canvas_center = response.rect.center();
                let pointer_offset = pos - canvas_center;
                let mut target_pan = -pointer_offset * (zoom_factor - 1.0);
                
                // AXIS-BASED CLAMPING: Clamp target_pan based on the target scale.
                // This prevents the image from sliding vertically/horizontally on an axis 
                // where it is still smaller than the window (e.g. wide images).
                let target_image_size = image_size * target_scale;
                let max_pan_x = ((target_image_size.x - canvas_size.x) / 2.0).max(0.0);
                let max_pan_y = ((target_image_size.y - canvas_size.y) / 2.0).max(0.0);
                target_pan.x = target_pan.x.clamp(-max_pan_x, max_pan_x);
                target_pan.y = target_pan.y.clamp(-max_pan_y, max_pan_y);
                
                state.target_pan = Some(target_pan);
            }
        } else {
            state.target_scale = Some(fit_scale);
            state.target_pan = Some(egui::Vec2::ZERO);
        }
        state.reset_start_time = Some(ui.input(|i| i.time));
    }

    if !state.frames.is_empty() {
        let current_time = ctx.input(|i| i.time);
        
        // Manual Zoom/Pan logic
        if state.reset_start_time.is_none() {
            if response.hovered() {
                let scroll = ctx.input(|i| i.smooth_scroll_delta.y); 
                if scroll != 0.0 {
                    if let Some(pos) = pointer_pos {
                        state.auto_fit = false;
                        let zoom_multiplier = (scroll * 0.005).exp();
                        let old_scale = state.scale;
                        let new_scale = (old_scale * zoom_multiplier).max(fit_scale);

                        let canvas_center = response.rect.center();
                        let pointer_offset = pos - canvas_center;
                        let scale_ratio = new_scale / old_scale;
                        
                        // SYNCED CALCULATION: Calculate and clamp pan simultaneously with scale
                        let mut new_pan = state.pan - (pointer_offset - state.pan) * (scale_ratio - 1.0);
                        
                        let scaled_size = image_size * new_scale;
                        let max_pan_x = ((scaled_size.x - canvas_size.x) / 2.0).max(0.0);
                        let max_pan_y = ((scaled_size.y - canvas_size.y) / 2.0).max(0.0);
                        new_pan.x = new_pan.x.clamp(-max_pan_x, max_pan_x);
                        new_pan.y = new_pan.y.clamp(-max_pan_y, max_pan_y);

                        state.pan = new_pan;
                        state.scale = new_scale;
                    }
                }
            }

            if is_zoomed_in {
                if response.dragged_by(egui::PointerButton::Primary) {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                    state.auto_fit = false;
                    state.pan += response.drag_delta();
                }
                
                // Final boundary check for active dragging
                let scaled_size = image_size * state.scale;
                let max_pan_x = ((scaled_size.x - canvas_size.x) / 2.0).max(0.0);
                let max_pan_y = ((scaled_size.y - canvas_size.y) / 2.0).max(0.0);
                state.pan.x = state.pan.x.clamp(-max_pan_x, max_pan_x);
                state.pan.y = state.pan.y.clamp(-max_pan_y, max_pan_y);
            }
        }

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

        // Smooth Interpolation for Zoom/Pan transitions
        if let Some(start_time) = state.reset_start_time {
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
        }

        // Draw the Image
        let texture = &state.frames[state.current_frame];
        let scaled_size = image_size * state.scale;
        let center_offset = (canvas_size - scaled_size) / 2.0;
        let image_top_left = response.rect.min + center_offset + state.pan;
        let draw_rect = egui::Rect::from_min_size(image_top_left, scaled_size);
        let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
        
        painter.image(texture.id(), draw_rect, uv, egui::Color32::WHITE);
    } else {
        // Draw loading or empty indicators
        let mut child_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(response.rect)
                .layout(egui::Layout::centered_and_justified(egui::Direction::TopDown))
        );
        
        if let Some(err) = &state.load_error {
            child_ui.add(
                egui::Label::new(egui::RichText::new(err).color(child_ui.visuals().error_fg_color))
                    .selectable(false),
            );
        } else if state.current_file_name.is_empty() {
            child_ui.add(egui::Label::new("No image loaded.\nDrag and drop an image here.").selectable(false));
        } else {
            child_ui.spinner();
        }
    }

    // Persistent Arrow Overlays
    // Rendered completely outside image-loading logic to prevent flashing
    if playlist_len > 1 && state.reset_start_time.is_none() && !state.current_file_name.is_empty() {
        if pointer_pos.is_some() {
            let center_y = response.rect.center().y;
            
            if has_prev && in_left_zone {
                let x_pos = response.rect.min.x + 40.0;
                painter.circle_filled(egui::pos2(x_pos, center_y), 24.0, egui::Color32::from_black_alpha(150));
                painter.text(egui::pos2(x_pos - 2.0, center_y), egui::Align2::CENTER_CENTER, egui_phosphor::regular::CARET_LEFT, egui::FontId::proportional(28.0), egui::Color32::WHITE);
            } else if has_next && in_right_zone {
                let x_pos = response.rect.max.x - 40.0; // FIXED: Changed + to - to bring it on screen
                painter.circle_filled(egui::pos2(x_pos, center_y), 24.0, egui::Color32::from_black_alpha(150));
                painter.text(egui::pos2(x_pos + 2.0, center_y), egui::Align2::CENTER_CENTER, egui_phosphor::regular::CARET_RIGHT, egui::FontId::proportional(28.0), egui::Color32::WHITE);
            }
        }
    }

    nav_action
}