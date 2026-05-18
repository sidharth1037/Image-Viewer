use eframe::egui;
use egui_phosphor::regular as icons;

const TOOLBAR_WIDTH: f32 = 40.0;
const BUTTON_SIZE: f32 = 30.0;
const TOOLBAR_PADDING: f32 = 6.0;
const ITEM_SPACING: f32 = 6.0;
const PANEL_ID: &str = "folder_toolbar_panel";
const SEPARATOR_ID: &str = "folder_toolbar_separator";

#[derive(Clone, Copy)]
pub enum FolderToolbarAction {
    ToggleRecursiveScan,
}

pub struct FolderToolbarButton {
    pub id: &'static str,
    pub icon: &'static str,
    pub tooltip: &'static str,
    pub selected: bool,
    pub enabled: bool,
    pub action: FolderToolbarAction,
}

pub fn default_buttons(recursive_scan_enabled: bool) -> [FolderToolbarButton; 1] {
    [FolderToolbarButton {
        id: "recursive_scan_toggle",
        icon: icons::FOLDER_OPEN,
        tooltip: "Include subfolders (recursive scan)",
        selected: recursive_scan_enabled,
        enabled: true,
        action: FolderToolbarAction::ToggleRecursiveScan,
    }]
}

pub fn render(ctx: &egui::Context, buttons: &[FolderToolbarButton]) -> Option<FolderToolbarAction> {
    let mut action = None;
    let panel_frame = egui::Frame::side_top_panel(&ctx.style())
        .inner_margin(egui::Margin::same(0))
        .fill(ctx.style().visuals.window_fill());

    let panel_response = egui::SidePanel::left(PANEL_ID)
        .resizable(false)
        .exact_width(TOOLBAR_WIDTH)
        .frame(panel_frame)
        .show(ctx, |ui| {
            ui.set_width(TOOLBAR_WIDTH);
            ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                ui.add_space(TOOLBAR_PADDING);
                ui.spacing_mut().item_spacing.y = ITEM_SPACING;

                for button in buttons {
                    let response = toolbar_icon_button(ui, button);
                    if response.clicked() && button.enabled {
                        action = Some(button.action);
                    }
                }

                ui.add_space(TOOLBAR_PADDING);
            });
        });

    let rect = panel_response.response.rect;
    let stroke = egui::Stroke::new(
        1.0,
        ctx.style().visuals.widgets.noninteractive.bg_stroke.color,
    );
    ctx.layer_painter(egui::LayerId::new(
        egui::Order::Middle,
        egui::Id::new(SEPARATOR_ID),
    ))
    .vline(rect.right(), rect.y_range(), stroke);

    action
}

fn toolbar_icon_button(ui: &mut egui::Ui, button: &FolderToolbarButton) -> egui::Response {
    let response = ui.push_id(button.id, |ui| {
        ui.add_enabled(
            button.enabled,
            egui::Button::new(button.icon)
                .min_size(egui::vec2(BUTTON_SIZE, BUTTON_SIZE))
                .selected(button.selected),
        )
    });

    let response = response.inner;

    if button.tooltip.is_empty() {
        return response;
    }

    if button.enabled {
        response.on_hover_text(button.tooltip)
    } else {
        response.on_disabled_hover_text(button.tooltip)
    }
}
