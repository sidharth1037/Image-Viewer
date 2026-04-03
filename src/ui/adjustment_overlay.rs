/// Renders a temporary on-screen overlay showing the latest adjustment or shortcut hint.
/// The overlay appears when a new hint is pushed, then fades out after a short duration.

use eframe::egui;
use crate::state::ViewerState;

/// How long the overlay stays fully visible before fading (seconds).
const HOLD_DURATION: f64 = 0.5;
/// How long the fade-out transition takes (seconds).
const FADE_DURATION: f64 = 1.5;
/// Total visible time = HOLD + FADE.
const TOTAL_DURATION: f64 = HOLD_DURATION + FADE_DURATION;

/// Top-left padding for the overlay position.
const TOP_PADDING: f32 = 48.0;  // Enough clearance for the 32px topbar + 16px extra
const LEFT_PADDING: f32 = 16.0;

/// Draws outlined text: black outline + shadow behind white text.
/// The outline is achieved by painting the text at 8 surrounding offsets.
fn paint_outlined_text(
    painter: &egui::Painter,
    pos: egui::Pos2,
    text: &str,
    font_id: egui::FontId,
    alpha: f32,
) {
    let outline_color = egui::Color32::from_rgba_unmultiplied(0, 0, 0, (255.0 * alpha) as u8);
    let text_color = egui::Color32::from_rgba_unmultiplied(255, 255, 255, (255.0 * alpha) as u8);
    let shadow_color = egui::Color32::from_rgba_unmultiplied(0, 0, 0, (120.0 * alpha) as u8);

    // 1. Shadow (offset down-right)
    painter.text(
        pos + egui::vec2(2.0, 2.0),
        egui::Align2::LEFT_TOP,
        text,
        font_id.clone(),
        shadow_color,
    );

    // 2. Black outline at 8 surrounding positions (1px offset)
    let offsets: [(f32, f32); 8] = [
        (-1.0, -1.0), (0.0, -1.0), (1.0, -1.0),
        (-1.0,  0.0),              (1.0,  0.0),
        (-1.0,  1.0), (0.0,  1.0), (1.0,  1.0),
    ];
    for (dx, dy) in offsets {
        painter.text(
            pos + egui::vec2(dx, dy),
            egui::Align2::LEFT_TOP,
            text,
            font_id.clone(),
            outline_color,
        );
    }

    // 3. White foreground text
    painter.text(
        pos,
        egui::Align2::LEFT_TOP,
        text,
        font_id,
        text_color,
    );
}

/// Renders the overlay if a hint was recently pushed.
/// Call this from the main update loop. It will automatically fade out and
/// request repaints only while the overlay is visible.
pub fn render(ctx: &egui::Context, state: &ViewerState) {
    let last_changed = match state.overlay_last_changed {
        Some(t) => t,
        None => return,
    };
    let text = match &state.overlay_text {
        Some(t) => t,
        None => return,
    };

    let current_time = ctx.input(|i| i.time);
    let elapsed = current_time - last_changed;

    // Fully expired — nothing to draw
    if elapsed >= TOTAL_DURATION {
        return;
    }

    // Calculate alpha: full opacity during hold, then linear fade
    let alpha = if elapsed < HOLD_DURATION {
        1.0
    } else {
        let fade_progress = (elapsed - HOLD_DURATION) / FADE_DURATION;
        (1.0 - fade_progress as f32).max(0.0)
    };

    // Paint on the foreground layer so it's always visible above the image
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("adjustment_overlay"),
    ));

    let pos = ctx.content_rect().min + egui::vec2(LEFT_PADDING, TOP_PADDING);

    paint_outlined_text(
        &painter,
        pos,
        text,
        egui::FontId::proportional(18.0),
        alpha,
    );

    // Request repaint while fading to keep animation smooth
    if elapsed < TOTAL_DURATION {
        ctx.request_repaint();
    }
}
