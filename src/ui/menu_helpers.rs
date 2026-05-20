use eframe::egui;

pub fn menu_row_button(ui: &mut egui::Ui, label: &str, tooltip: &str, selected: bool) -> bool {
    const H_PADDING: f32 = 10.0;

    let row_size = egui::vec2(ui.available_width(), ui.spacing().interact_size.y);
    let (rect, response) = ui.allocate_exact_size(row_size, egui::Sense::click());
    let response = if tooltip.is_empty() {
        response
    } else {
        response.on_hover_text(tooltip)
    };

    if ui.is_rect_visible(rect) {
        let visuals = &ui.style().visuals;
        let widget = if selected {
            &visuals.widgets.active
        } else if response.hovered() {
            &visuals.widgets.hovered
        } else {
            &visuals.widgets.inactive
        };

        ui.painter().rect_filled(rect, widget.corner_radius, widget.bg_fill);

        ui.painter().text(
            egui::pos2(rect.left() + H_PADDING, rect.center().y),
            egui::Align2::LEFT_CENTER,
            label,
            egui::TextStyle::Button.resolve(ui.style()),
            widget.fg_stroke.color,
        );
    }

    response.clicked()
}

