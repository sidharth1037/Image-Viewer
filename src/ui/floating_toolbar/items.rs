use crate::app::ImageApp;
use crate::sync;
use eframe::egui;
use egui_phosphor::regular as icons;

pub fn render_carry_adjustments_toggle(app: &mut ImageApp, ui: &mut egui::Ui) {
    let view = app.workspace.active_view();
    let selected = view.carry_adjustments;

    let response = toolbar_button(
        ui,
        icons::COPY,
        selected,
        true,
        "Carry adjustments to next image",
    );

    if response.clicked() {
        app.workspace.active_view_mut().carry_adjustments = !selected;
    }
}

pub fn render_split_pan_zoom_sync_toggle(app: &mut ImageApp, ui: &mut egui::Ui) {
    if !app.workspace.is_split() {
        return;
    }

    let mismatch_reason = sync::pan_zoom::aspect_ratio_mismatch_reason(app);
    let can_enable = mismatch_reason.is_none();
    let tooltip = build_sync_tooltip(mismatch_reason);

    let response = toolbar_button(
        ui,
        icons::ARROWS_LEFT_RIGHT,
        app.split_pan_zoom_sync_enabled,
        can_enable,
        &tooltip,
    );

    if response.clicked() && can_enable {
        let next = !app.split_pan_zoom_sync_enabled;
        app.split_pan_zoom_sync_enabled = next;
        app.split_pan_zoom_sync_user_disabled = !next;
    }
}

fn build_sync_tooltip(reason: Option<String>) -> String {
    match reason {
        Some(text) => format!("Sync zoom/pan\n{}", text),
        None => "Sync zoom/pan".to_string(),
    }
}

fn toolbar_button(
    ui: &mut egui::Ui,
    icon: &str,
    selected: bool,
    enabled: bool,
    tooltip: &str,
) -> egui::Response {
    let response = ui.add_enabled(enabled, egui::Button::new(icon).selected(selected));
    if tooltip.is_empty() {
        return response;
    }
    if enabled {
        response.on_hover_text(tooltip)
    } else {
        response.on_disabled_hover_text(tooltip)
    }
}
