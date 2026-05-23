use crate::app::ImageApp;
use crate::groups::{ASK_EVERY_TIME_LABEL, GroupAssignTarget};
use crate::ui::menu_helpers;
use crate::sync;
use eframe::egui;
use egui_phosphor::regular as icons;

const GROUP_ASSIGN_MENU_ID: &str = "group_assign_menu";
const GROUP_ASSIGN_MENU_WIDTH: f32 = 210.0;
const GROUP_ASSIGN_MENU_GAP: f32 = 4.0;

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

pub fn render_group_assign_dropdown(app: &mut ImageApp, ctx: &egui::Context, ui: &mut egui::Ui) {
    if !app.settings.groups_enabled {
        return;
    }

    let resolved_target = resolve_group_assign_target(app);
    let label = group_assign_label(app, resolved_target);
    let button_label = format!("{} {}", icons::TAG, label);
    let tooltip = "Add current image to a group [M]";

    let response = ui.button(button_label).on_hover_text(tooltip);

    if response.clicked() {
        if app.show_group_assign_menu {
            app.show_group_assign_menu = false;
            app.group_assign_menu_pos = None;
        } else {
            app.show_group_assign_menu = true;
            app.group_assign_menu_pos = Some(group_assign_menu_pos(ui, response.rect, app));
        }
    }

    if !app.show_group_assign_menu {
        return;
    }

    let popup_pos = app
        .group_assign_menu_pos
        .unwrap_or_else(|| group_assign_menu_pos(ui, response.rect, app));

    let mut selection_changed = false;

    let area_res = egui::Area::new(egui::Id::new(GROUP_ASSIGN_MENU_ID))
        .fixed_pos(popup_pos)
        .order(egui::Order::Tooltip)
        .show(ctx, |ui| {
            egui::Frame::menu(ui.style()).show(ui, |ui| {
                ui.set_width(GROUP_ASSIGN_MENU_WIDTH);

                let ask_selected = matches!(resolved_target, GroupAssignTarget::AskEveryTime);
                if menu_helpers::menu_row_button(ui, ASK_EVERY_TIME_LABEL, "", ask_selected) {
                    app.group_assign_target = GroupAssignTarget::AskEveryTime;
                    selection_changed = true;
                }

                for group in app.workspace.group_tabs.user_groups.iter() {
                    let is_selected = matches!(resolved_target, GroupAssignTarget::Group(id) if id == group.id);
                    if menu_helpers::menu_row_button(ui, &group.name, "", is_selected) {
                        app.group_assign_target = GroupAssignTarget::Group(group.id);
                        selection_changed = true;
                    }
                }
            });
        });

    if selection_changed {
        app.show_group_assign_menu = false;
        app.group_assign_menu_pos = None;
    }

    if app.show_group_assign_menu && ctx.input(|i| i.pointer.any_pressed()) {
        let clicked_outside = ctx.pointer_interact_pos().is_some_and(|pos| {
            !area_res.response.rect.contains(pos) && !response.rect.contains(pos)
        });
        if clicked_outside {
            app.show_group_assign_menu = false;
            app.group_assign_menu_pos = None;
        }
    }
}

fn build_sync_tooltip(reason: Option<String>) -> String {
    match reason {
        Some(text) => format!("Sync zoom/pan\n{}", text),
        None => "Sync zoom/pan".to_string(),
    }
}

fn group_assign_label(app: &ImageApp, target: GroupAssignTarget) -> String {
    match target {
        GroupAssignTarget::AskEveryTime => ASK_EVERY_TIME_LABEL.to_string(),
        GroupAssignTarget::Group(id) => app
            .workspace
            .group_tabs
            .group_name(id)
            .unwrap_or_else(|| ASK_EVERY_TIME_LABEL.to_string()),
    }
}

fn resolve_group_assign_target(app: &ImageApp) -> GroupAssignTarget {
    match app.group_assign_target {
        GroupAssignTarget::Group(id) if app.workspace.group_tabs.has_group(id) => {
            app.group_assign_target
        }
        GroupAssignTarget::Group(_) => GroupAssignTarget::AskEveryTime,
        GroupAssignTarget::AskEveryTime => GroupAssignTarget::AskEveryTime,
    }
}

fn group_assign_menu_pos(ui: &egui::Ui, button_rect: egui::Rect, app: &ImageApp) -> egui::Pos2 {
    let item_count = 1 + app.workspace.group_tabs.user_groups.len();
    let row_h = ui.spacing().interact_size.y;
    let menu_h = item_count as f32 * row_h + GROUP_ASSIGN_MENU_GAP;

    egui::pos2(
        button_rect.left(),
        (button_rect.top() - GROUP_ASSIGN_MENU_GAP - menu_h).max(0.0),
    )
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
