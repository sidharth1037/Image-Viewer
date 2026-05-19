use eframe::egui;
use egui_phosphor::regular as icons;

use crate::app::ImageApp;
use crate::groups::{DEFAULT_GROUP_ID, DEFAULT_GROUP_NAME};
use crate::handlers;

const GROUP_TABS_HEIGHT: f32 = 30.0;
const TAB_HEIGHT: f32 = 24.0;
const TAB_MIN_WIDTH: f32 = 80.0;
const TAB_PADDING_X: f32 = 10.0;
const TAB_SPACING_X: f32 = 6.0;
const TAB_CORNER_RADIUS: f32 = 6.0;
const CLOSE_AREA_WIDTH: f32 = 18.0;
const ADD_BUTTON_WIDTH: f32 = 34.0;
const ADD_BUTTON_SIZE: f32 = 24.0;

pub fn tabs_height(app: &ImageApp) -> f32 {
    if is_visible(app) {
        GROUP_TABS_HEIGHT
    } else {
        0.0
    }
}

pub fn render_in_rect(app: &mut ImageApp, _ctx: &egui::Context, ui: &mut egui::Ui, rect: egui::Rect) {
    if !is_visible(app) {
        return;
    }

    let visuals = ui.style().visuals.clone();
    let bg_fill = visuals.window_fill();

    ui.painter().rect_filled(rect, 0.0, bg_fill);

    let separator_stroke = egui::Stroke::new(1.0, visuals.widgets.noninteractive.bg_stroke.color);
    ui.painter()
        .hline(rect.x_range(), rect.bottom(), separator_stroke);

    let mut add_rect = rect;
    add_rect.max.x = (rect.min.x + ADD_BUTTON_WIDTH).min(rect.max.x);

    let mut tabs_rect = rect;
    tabs_rect.min.x = add_rect.max.x + TAB_SPACING_X;
    if tabs_rect.min.x > tabs_rect.max.x {
        tabs_rect.min.x = rect.min.x;
    }

    let row_rect = egui::Rect::from_min_max(
        egui::pos2(tabs_rect.min.x + TAB_SPACING_X, rect.center().y - TAB_HEIGHT * 0.5),
        egui::pos2(tabs_rect.max.x - TAB_SPACING_X, rect.center().y + TAB_HEIGHT * 0.5),
    );

    ui.scope_builder(egui::UiBuilder::new().max_rect(row_rect), |ui| {
        ui.set_height(TAB_HEIGHT);
        ui.spacing_mut().item_spacing.x = TAB_SPACING_X;
        ui.spacing_mut().item_spacing.y = 0.0;

        let groups: Vec<(u32, String)> = app
            .workspace
            .group_tabs
            .user_groups
            .iter()
            .map(|group| (group.id, group.name.clone()))
            .collect();

        let mut pending_close: Option<u32> = None;
        let mut pending_select: Option<u32> = None;
        let mut pending_drop: Option<u32> = None;
        let drag_active = app.group_drag_payload.is_some();
        let pointer_released = ui.input(|i| i.pointer.any_released());

        egui::ScrollArea::horizontal()
            .id_salt("group_tabs_scroll")
            .auto_shrink([false, false])
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let default_selected = app.workspace.group_tabs.is_selected(DEFAULT_GROUP_ID);
                    let default_action = render_tab(ui, DEFAULT_GROUP_ID, DEFAULT_GROUP_NAME, default_selected, false);
                    if drag_active && pointer_released && default_action.response.hovered() {
                        pending_drop = Some(DEFAULT_GROUP_ID);
                    }
                    if default_action.select_clicked {
                        pending_select = Some(DEFAULT_GROUP_ID);
                    }

                    for (group_id, group_name) in groups.iter() {
                        let is_selected = app.workspace.group_tabs.is_selected(*group_id);
                        let action = render_tab(ui, *group_id, group_name, is_selected, true);

                        if drag_active && pointer_released && action.response.hovered() {
                            pending_drop = Some(*group_id);
                        }

                        if action.close_clicked {
                            pending_close = Some(*group_id);
                        } else if action.select_clicked {
                            pending_select = Some(*group_id);
                        }
                    }
                });
            });

        if let Some(group_id) = pending_drop {
            let time = ui.input(|i| i.time);
            handlers::handle_group_drop(app, group_id, time);
        } else if let Some(group_id) = pending_close {
            handlers::close_group_tab(app, group_id);
        } else if let Some(group_id) = pending_select {
            handlers::switch_group(app, group_id);
        }
    });

    let add_separator_x = add_rect.max.x;
    if add_separator_x > rect.min.x && add_separator_x < rect.max.x {
        ui.painter().vline(add_separator_x, rect.y_range(), separator_stroke);
    }

    ui.scope_builder(egui::UiBuilder::new().max_rect(add_rect), |ui| {
        ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
            ui.add_space((add_rect.height() - TAB_HEIGHT) * 0.5);
            
            let font_id = egui::TextStyle::Button.resolve(ui.style());
            let (button_rect, response) = ui.allocate_exact_size(egui::vec2(ADD_BUTTON_SIZE, TAB_HEIGHT), egui::Sense::click());
            
            let visuals = ui.style().visuals.clone();
            let (bg_fill, bg_stroke, fg_color) = if response.hovered() {
                (
                    visuals.widgets.hovered.bg_fill,
                    visuals.widgets.hovered.bg_stroke,
                    visuals.widgets.hovered.fg_stroke.color,
                )
            } else {
                (
                    visuals.widgets.inactive.bg_fill,
                    visuals.widgets.inactive.bg_stroke,
                    visuals.widgets.inactive.fg_stroke.color,
                )
            };

            ui.painter().rect_filled(button_rect, TAB_CORNER_RADIUS, bg_fill);
            ui.painter().rect_stroke(button_rect, TAB_CORNER_RADIUS, bg_stroke, egui::StrokeKind::Inside);
            ui.painter().text(
                button_rect.center(),
                egui::Align2::CENTER_CENTER,
                icons::PLUS,
                font_id,
                fg_color,
            );

            if response.on_hover_text("New group").clicked() {
                app.workspace.group_tabs.add_group();
            }
        });
    });
}

fn is_visible(app: &ImageApp) -> bool {
    app.settings.groups_enabled
        && app.workspace.content_mode == crate::workspace::ContentMode::PlaylistGrid
}

struct TabAction {
    response: egui::Response,
    select_clicked: bool,
    close_clicked: bool,
}

fn render_tab(ui: &mut egui::Ui, id: u32, label: &str, selected: bool, closable: bool) -> TabAction {
    let font_id = egui::TextStyle::Button.resolve(ui.style());
    let label_size = ui
        .painter()
        .layout_no_wrap(label.to_string(), font_id.clone(), egui::Color32::WHITE)
        .size();
    let label_width = label_size.x;
    let label_height = label_size.y;

    let mut width = (label_width + TAB_PADDING_X * 2.0).max(TAB_MIN_WIDTH);
    if closable {
        width += CLOSE_AREA_WIDTH;
    }

    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, TAB_HEIGHT), egui::Sense::click());

    let visuals = ui.style().visuals.clone();
    let (bg_fill, bg_stroke, fg_color) = if selected {
        (
            visuals.widgets.active.bg_fill,
            visuals.widgets.active.bg_stroke,
            visuals.widgets.active.fg_stroke.color,
        )
    } else if response.hovered() {
        (
            visuals.widgets.hovered.bg_fill,
            visuals.widgets.hovered.bg_stroke,
            visuals.widgets.hovered.fg_stroke.color,
        )
    } else {
        (
            visuals.widgets.inactive.bg_fill,
            visuals.widgets.inactive.bg_stroke,
            visuals.widgets.inactive.fg_stroke.color,
        )
    };

    ui.painter()
        .rect_filled(rect, TAB_CORNER_RADIUS, bg_fill);
    ui.painter().rect_stroke(
        rect,
        TAB_CORNER_RADIUS,
        bg_stroke,
        egui::StrokeKind::Inside,
    );

    let text_pos = egui::pos2(
        rect.min.x + TAB_PADDING_X,
        rect.center().y - label_height * 0.5,
    );
    ui.painter().text(
        text_pos,
        egui::Align2::LEFT_TOP,
        label,
        font_id.clone(),
        fg_color,
    );

    let mut close_clicked = false;
    if closable {
        let close_rect = egui::Rect::from_min_max(
            egui::pos2(rect.max.x - CLOSE_AREA_WIDTH, rect.min.y),
            rect.max,
        );
        let close_id = ui.id().with(("close", id));
        let close_response = ui.interact(close_rect, close_id, egui::Sense::click());

        let close_color = if close_response.hovered() {
            visuals.widgets.hovered.fg_stroke.color
        } else {
            fg_color
        };

        ui.painter().text(
            close_rect.center(),
            egui::Align2::CENTER_CENTER,
            icons::X,
            font_id,
            close_color,
        );

        close_clicked = close_response.clicked();
    }

    let select_clicked = response.clicked() && !close_clicked;

    TabAction {
        response,
        select_clicked,
        close_clicked,
    }
}
