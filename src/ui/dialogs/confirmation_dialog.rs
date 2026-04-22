use eframe::egui;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfirmationSelection {
    Cancel,
    Confirm,
}

#[derive(Clone, Copy)]
pub struct ConfirmationDialogSpec<'a> {
    pub id_source: &'a str,
    pub title: &'a str,
    pub message: &'a str,
    pub cancel_label: &'a str,
    pub confirm_label: &'a str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfirmationDialogAction {
    Cancel,
    Confirm,
}

const DIALOG_WIDTH: f32 = 460.0;
const BUTTON_WIDTH: f32 = 140.0;
const BUTTON_HEIGHT: f32 = 34.0;

pub fn render(
    ctx: &egui::Context,
    spec: &ConfirmationDialogSpec<'_>,
    selected: ConfirmationSelection,
    backdrop_rect: egui::Rect,
) -> Option<ConfirmationDialogAction> {
    paint_modal_backdrop(ctx, spec.id_source, backdrop_rect);

    let mut action = None;

    egui::Window::new(spec.title)
        .id(egui::Id::new((spec.id_source, "window")))
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .collapsible(false)
        .resizable(false)
        .movable(false)
        .show(ctx, |ui| {
            ui.set_min_width(DIALOG_WIDTH);

            ui.add(egui::Label::new(spec.message).wrap());
            ui.add_space(14.0);

            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 10.0;

                let cancel_clicked = render_action_button(
                    ui,
                    spec.cancel_label,
                    selected == ConfirmationSelection::Cancel,
                    false,
                )
                .clicked();

                let confirm_clicked = render_action_button(
                    ui,
                    spec.confirm_label,
                    selected == ConfirmationSelection::Confirm,
                    true,
                )
                .clicked();

                if cancel_clicked {
                    action = Some(ConfirmationDialogAction::Cancel);
                }
                if confirm_clicked {
                    action = Some(ConfirmationDialogAction::Confirm);
                }
            });
        });

    action
}

fn paint_modal_backdrop(ctx: &egui::Context, id_source: &str, backdrop_rect: egui::Rect) {
    if !backdrop_rect.is_positive() {
        return;
    }

    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new((id_source, "backdrop_painter")),
    ));
    painter.rect_filled(backdrop_rect, 0.0, egui::Color32::from_black_alpha(145));
}

fn render_action_button(
    ui: &mut egui::Ui,
    label: &str,
    selected: bool,
    destructive: bool,
) -> egui::Response {
    let mut button = egui::Button::new(label).min_size(egui::vec2(BUTTON_WIDTH, BUTTON_HEIGHT));

    if selected && destructive {
        button = button
            .fill(egui::Color32::from_rgb(153, 44, 44))
            .stroke(egui::Stroke::new(2.0, egui::Color32::from_rgb(214, 92, 92)));
    } else if selected {
        button = button
            .fill(ui.visuals().selection.bg_fill)
            .stroke(egui::Stroke::new(2.0, ui.visuals().selection.stroke.color));
    }

    ui.add(button)
}
