use crate::state::ViewerState;
use eframe::egui;

const DOUBLE_CLICK_ZOOM_LEVEL: f32 = 2.5;
pub const MAX_ZOOM_MULTIPLIER: f32 = 5.0;

#[derive(Clone, Copy)]
pub struct ZoomMetrics {
    pub actual_scale: f32,
    pub fit_scale: f32,
    pub min_zoom_scale: f32,
    pub default_display_scale: f32,
    pub max_zoom_scale: f32,
    pub current_percent: f32,
    pub min_percent: f32,
    pub max_percent: f32,
}

pub fn compute_zoom_metrics(
    ctx: &egui::Context,
    state: &ViewerState,
    canvas_size: egui::Vec2,
    fit_all_images_to_window: bool,
    pixel_based_1_to_1: bool,
) -> Option<ZoomMetrics> {
    if state.frames.is_empty() || canvas_size.x <= 0.0 || canvas_size.y <= 0.0 {
        return None;
    }

    let texture = &state.frames[state.current_frame];
    let image_pixels = texture.size_vec2();
    let pixels_per_point = ctx.pixels_per_point().max(0.0001);
    let image_size = image_pixels / pixels_per_point;
    if image_size.x <= 0.0 || image_size.y <= 0.0 {
        return None;
    }

    let pixel_1_to_1_scale = 1.0;
    // egui logical points are defined as 72 points per inch.
    let monitor_ppi = ctx.pixels_per_point() * 72.0;
    let true_size_scale = state
        .image_density
        .map(|density| {
            let _ = &density.source;
            let image_ppi = density.average_ppi();
            (monitor_ppi / image_ppi).clamp(0.01, 100.0)
        })
        .unwrap_or(pixel_1_to_1_scale);
    let actual_scale = if pixel_based_1_to_1 {
        pixel_1_to_1_scale
    } else {
        true_size_scale
    };

    let scale_w = canvas_size.x / image_size.x;
    let scale_h = canvas_size.y / image_size.y;
    let fit_scale = scale_w.min(scale_h);
    let min_zoom_scale = fit_scale.min(actual_scale);
    let is_small_image = fit_scale > actual_scale;
    let default_display_scale = if fit_all_images_to_window || !is_small_image {
        fit_scale
    } else {
        actual_scale
    };
    let max_zoom_scale = actual_scale * MAX_ZOOM_MULTIPLIER;

    let actual_scale_safe = actual_scale.max(0.0001);
    let current_percent = (state.scale / actual_scale_safe) * 100.0;
    let min_percent = (min_zoom_scale / actual_scale_safe) * 100.0;
    let max_percent = MAX_ZOOM_MULTIPLIER * 100.0;

    Some(ZoomMetrics {
        actual_scale,
        fit_scale,
        min_zoom_scale,
        default_display_scale,
        max_zoom_scale,
        current_percent,
        min_percent,
        max_percent,
    })
}

pub fn reset_view_for_mode_change(
    ctx: &egui::Context,
    state: &mut ViewerState,
    canvas_size: egui::Vec2,
    fit_all_images_to_window: bool,
    pixel_based_1_to_1: bool,
) {
    state.auto_fit = true;
    state.pan = egui::Vec2::ZERO;
    state.target_scale = None;
    state.target_pan = None;
    state.reset_start_time = None;

    if let Some(metrics) = compute_zoom_metrics(
        ctx,
        state,
        canvas_size,
        fit_all_images_to_window,
        pixel_based_1_to_1,
    ) {
        state.scale = metrics.default_display_scale;
    }
}

/// Height of the blue focus indicator strip in split view.
const FOCUS_INDICATOR_HEIGHT: f32 = 3.0;
/// Height of the immersive topbar overlay (including margins/strokes)
const IMMERSIVE_TOPBAR_HEIGHT: f32 = 36.0;

pub fn render(
    ctx: &egui::Context,
    ui: &mut egui::Ui,
    state: &mut ViewerState,
    loop_playlist: bool,
    fit_all_images_to_window: bool,
    pixel_based_1_to_1: bool,
    is_active: bool,
    is_split: bool,
    immersive_topbar_visible: bool,
) -> Option<i32> {
    let mut nav_action = None;

    // In split view, allocate a focus indicator strip before the canvas.
    // This pushes the canvas content down so the indicator never overlaps the image.
    let show_focus_indicator = is_split && is_active;
    let mut indicator_x_range = None;

    if is_split {
        let strip_size = egui::vec2(ui.available_width(), FOCUS_INDICATOR_HEIGHT);
        let (strip_rect, _) = ui.allocate_exact_size(strip_size, egui::Sense::hover());

        if show_focus_indicator && !immersive_topbar_visible {
            let line_color = egui::Color32::from_rgb(0, 122, 204);
            ui.painter().rect_filled(strip_rect, 0.0, line_color);
        }

        indicator_x_range = Some(strip_rect.x_range());
    }

    // Allocate a persistent interaction area for the entire canvas.
    // This ensures inputs are captured consistently even during image transitions.
    let canvas_size = ui.available_size();
    state.last_canvas_size = canvas_size;
    let (response, painter) = ui.allocate_painter(canvas_size, egui::Sense::click_and_drag());
    painter.rect_filled(response.rect, 0.0, ui.visuals().window_fill());

    // When immersive topbar is visible, draw the focus indicator at the topbar's bottom
    // edge using a foreground painter so it stays visible above the floating overlay.
    if show_focus_indicator && immersive_topbar_visible {
        if let Some(x_range) = indicator_x_range {
            let line_color = egui::Color32::from_rgb(0, 122, 204);
            let line_stroke = egui::Stroke::new(FOCUS_INDICATOR_HEIGHT, line_color);
            let y = IMMERSIVE_TOPBAR_HEIGHT + FOCUS_INDICATOR_HEIGHT * 0.5;
            ctx.layer_painter(egui::LayerId::new(
                egui::Order::Foreground,
                egui::Id::new("split_focus_indicator"),
            ))
            .hline(x_range, y, line_stroke);
        }
    }

    let mut fit_scale = 1.0;
    let mut min_zoom_scale = 1.0;
    let mut max_zoom_scale = MAX_ZOOM_MULTIPLIER;
    let mut default_display_scale = 1.0;
    let mut actual_scale = 1.0;
    let mut image_size = egui::Vec2::ZERO;

    if !state.frames.is_empty() {
        let texture = &state.frames[state.current_frame];
        let image_pixels = texture.size_vec2();
        let pixels_per_point = ctx.pixels_per_point().max(0.0001);
        image_size = image_pixels / pixels_per_point;

        let metrics = compute_zoom_metrics(
            ctx,
            state,
            canvas_size,
            fit_all_images_to_window,
            pixel_based_1_to_1,
        );
        if let Some(m) = metrics {
            fit_scale = m.fit_scale;
            min_zoom_scale = m.min_zoom_scale;
            max_zoom_scale = m.max_zoom_scale;
            default_display_scale = m.default_display_scale;
            actual_scale = m.actual_scale;
        }

        // When auto_fit is enabled, lock to the current baseline for this image.
        if state.auto_fit {
            state.scale = default_display_scale;
            state.pan = egui::Vec2::ZERO;
        }
    }

    // Only track pointer while the canvas interaction region actually owns it.
    // This prevents overlay UI (top/bottom bars) from leaking clicks into navigation.
    let pointer_pos = response.interact_pointer_pos().or(response.hover_pos());
    let left_zone_bound = response.rect.min.x + canvas_size.x * 0.08;
    let right_zone_bound = response.rect.max.x - canvas_size.x * 0.08;
    let pointer_in_canvas = response.contains_pointer();
    let pointer_in_immersive_topbar_overlay = immersive_topbar_visible
        && pointer_pos.is_some_and(|p| p.y <= IMMERSIVE_TOPBAR_HEIGHT);

    let in_left_zone = pointer_in_canvas
        && !pointer_in_immersive_topbar_overlay
        && pointer_pos.map_or(false, |p| p.x < left_zone_bound);
    let in_right_zone = pointer_in_canvas
        && !pointer_in_immersive_topbar_overlay
        && pointer_pos.map_or(false, |p| p.x > right_zone_bound);
    let in_nav_zone = in_left_zone || in_right_zone;

    // Keep edge affordance consistent: always show hand cursor in navigation zones.
    if in_nav_zone {
        ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    let is_zoomed_in = state.scale > default_display_scale * 1.0001;
    let image_fits_canvas = !state.frames.is_empty()
        && (image_size.x * state.scale) <= canvas_size.x + 0.5
        && (image_size.y * state.scale) <= canvas_size.y + 0.5;

    #[cfg(windows)]
    if response.drag_started_by(egui::PointerButton::Primary)
        && image_fits_canvas
        && !in_nav_zone
        && state.reset_start_time.is_none()
    {
        if let Some(path) = state.current_file_path.as_ref() {
            let _ = crate::platform::windows_drag_out::begin_file_drag(path.as_path());
        }
    }

    // Re-engage auto_fit once the user manually returns to the default baseline.
    if !state.frames.is_empty() && !state.auto_fit && state.reset_start_time.is_none() {
        let at_default_scale = (state.scale - default_display_scale).abs() < 0.001;
        if at_default_scale {
            state.auto_fit = true;
            state.scale = default_display_scale;
            state.pan = egui::Vec2::ZERO;
        }
    }

    // Determine navigation availability based on playlist position.
    let playlist_len = state.active_playlist.len();
    let current_idx = state.current_index;
    let has_prev = playlist_len > 1 && (loop_playlist || current_idx > 0);
    let has_next = playlist_len > 1 && (loop_playlist || current_idx + 1 < playlist_len);

    // Navigation Input: Handles clicks and double-clicks within the 8% edge zones.
    // Double clicks in these zones will navigate twice rather than triggering zoom.
    if state.reset_start_time.is_none() {
        if response.clicked() || response.double_clicked() {
            if !is_active {
                // Just absorb the click to assign focus. Return Some(0) which navigate() handles.
                nav_action = Some(0);
            } else if in_left_zone && has_prev {
                nav_action = Some(-1);
            } else if in_right_zone && has_next {
                nav_action = Some(1);
            }
        }
    }

    // Zoom Input: Handles double-click actions for fitted vs non-fitted states.
    // This is disabled in navigation zones when zoomed out to prevent accidental scale changes.
    if response.double_clicked() && (!in_nav_zone || is_zoomed_in) && !state.frames.is_empty() && is_active {
        state.auto_fit = false;
        let is_fitted = (state.scale - fit_scale).abs() < 0.001;
        let is_small_image = fit_scale > actual_scale;

        if is_small_image {
            if is_fitted {
                state.target_scale = Some(actual_scale);
            } else {
                state.target_scale = Some(fit_scale);
            }
            state.target_pan = Some(egui::Vec2::ZERO);
        } else if is_fitted {
            let zoom_factor = DOUBLE_CLICK_ZOOM_LEVEL;
            let target_scale = (fit_scale * zoom_factor).min(max_zoom_scale);
            state.target_scale = Some(target_scale);

            let mut target_pan = egui::Vec2::ZERO;
            if let Some(pos) = pointer_pos {
                let canvas_center = response.rect.center();
                let pointer_offset = pos - canvas_center;
                target_pan = -pointer_offset * (zoom_factor - 1.0);

                // Clamp pan on each axis where the target image exceeds canvas bounds.
                let target_image_size = image_size * target_scale;
                let max_pan_x = ((target_image_size.x - canvas_size.x) / 2.0).max(0.0);
                let max_pan_y = ((target_image_size.y - canvas_size.y) / 2.0).max(0.0);
                target_pan.x = target_pan.x.clamp(-max_pan_x, max_pan_x);
                target_pan.y = target_pan.y.clamp(-max_pan_y, max_pan_y);
            }

            state.target_pan = Some(target_pan);
        } else {
            state.target_scale = Some(fit_scale);
            state.target_pan = Some(egui::Vec2::ZERO);
        }

        state.reset_start_time = Some(ui.input(|i| i.time));
    }

    if !state.frames.is_empty() {
        let current_time = ctx.input(|i| i.time);

        // Manual Zoom/Pan logic.
        if state.reset_start_time.is_none() {
            if response.hovered() {
                let scroll = ctx.input(|i| i.smooth_scroll_delta.y);
                if scroll != 0.0 {
                    if let Some(pos) = pointer_pos {
                        state.auto_fit = false;
                        let zoom_multiplier = (scroll * 0.005).exp();
                        let old_scale = state.scale;
                        let new_scale = (old_scale * zoom_multiplier)
                            .max(min_zoom_scale)
                            .min(max_zoom_scale);

                        let canvas_center = response.rect.center();
                        let pointer_offset = pos - canvas_center;
                        let scale_ratio = new_scale / old_scale;

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

        // Smooth interpolation for zoom/pan transitions.
        if let Some(start_time) = state.reset_start_time {
            let elapsed = (current_time - start_time) as f32;
            let t = (elapsed / 0.35).clamp(0.0, 1.0);

            if let (Some(t_scale), Some(t_pan)) = (state.target_scale, state.target_pan) {
                let lerp_factor = 0.25;
                state.scale += (t_scale - state.scale) * lerp_factor;
                state.pan += (t_pan - state.pan) * lerp_factor;

                if t >= 1.0 || ((t_scale - state.scale).abs() < 0.001 && (t_pan - state.pan).length() < 0.1)
                {
                    state.scale = t_scale;
                    state.pan = t_pan;
                    state.reset_start_time = None;
                    state.target_scale = None;
                    state.target_pan = None;

                    if (t_scale - default_display_scale).abs() < 0.001 {
                        state.auto_fit = true;
                    }
                }
            }
            ctx.request_repaint();
        }

        // Draw the image.
        let texture = &state.frames[state.current_frame];
        let scaled_size = image_size * state.scale;
        let center_offset = (canvas_size - scaled_size) / 2.0;
        let image_top_left = response.rect.min + center_offset + state.pan;
        let draw_rect = egui::Rect::from_min_size(image_top_left, scaled_size);
        let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));

        painter.image(texture.id(), draw_rect, uv, egui::Color32::WHITE);
    } else {
        // Draw loading or empty indicators.
        let mut child_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(response.rect)
                .layout(egui::Layout::centered_and_justified(egui::Direction::TopDown)),
        );

        if let Some(err) = &state.load_error {
            child_ui.add(
                egui::Label::new(egui::RichText::new(err).color(child_ui.visuals().error_fg_color))
                    .selectable(false),
            );
        } else if state.current_file_name.is_empty() {
            child_ui
                .add(egui::Label::new("No image loaded.\nDrag and drop an image here.").selectable(false));
        } else {
            child_ui.spinner();
        }
    }

    // Persistent Arrow Overlays
    // Rendered completely outside image-loading logic to prevent flashing.
    if playlist_len > 1 && state.reset_start_time.is_none() && !state.current_file_name.is_empty() {
        if pointer_pos.is_some() {
            let center_y = response.rect.center().y;

            if has_prev && in_left_zone {
                let x_pos = response.rect.min.x + 40.0;
                painter.circle_filled(
                    egui::pos2(x_pos, center_y),
                    24.0,
                    egui::Color32::from_black_alpha(150),
                );
                painter.text(
                    egui::pos2(x_pos - 2.0, center_y),
                    egui::Align2::CENTER_CENTER,
                    egui_phosphor::regular::CARET_LEFT,
                    egui::FontId::proportional(28.0),
                    egui::Color32::WHITE,
                );
            } else if has_next && in_right_zone {
                let x_pos = response.rect.max.x - 40.0;
                painter.circle_filled(
                    egui::pos2(x_pos, center_y),
                    24.0,
                    egui::Color32::from_black_alpha(150),
                );
                painter.text(
                    egui::pos2(x_pos + 2.0, center_y),
                    egui::Align2::CENTER_CENTER,
                    egui_phosphor::regular::CARET_RIGHT,
                    egui::FontId::proportional(28.0),
                    egui::Color32::WHITE,
                );
            }
        }
    }

    nav_action
}
