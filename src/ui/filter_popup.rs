use crate::app::ImageApp;
use eframe::egui;

const FILTER_POPUP_ID: &str = "filter_popup";
const FILTER_TEXT_ID: &str = "filter_popup_text";
const FILTER_POPUP_WIDTH: f32 = 560.0;
const FILTER_POPUP_TOP_OFFSET: f32 = 44.0;

pub fn render(app: &mut ImageApp, ctx: &egui::Context) {
    if !app.show_filter_popup {
        return;
    }

    let screen_rect = ctx.content_rect();
    let popup_x = screen_rect.center().x - FILTER_POPUP_WIDTH * 0.5;
    let popup_pos = egui::pos2(popup_x.max(screen_rect.left() + 8.0), screen_rect.top() + FILTER_POPUP_TOP_OFFSET);

    let mut close_on_enter = false;

    let area_res = egui::Area::new(egui::Id::new(FILTER_POPUP_ID))
        .fixed_pos(popup_pos)
        .order(egui::Order::Tooltip)
        .show(ctx, |ui| {
            egui::Frame::menu(ui.style()).show(ui, |ui| {
                ui.set_width(FILTER_POPUP_WIDTH);
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new("Filter Playlist").strong());
                    ui.add_space(6.0);

                    let mut filter_text = app.state.filter.criteria.text.clone();
                    let text_id = egui::Id::new(FILTER_TEXT_ID);
                    let text_res = ui.add(
                        egui::TextEdit::singleline(&mut filter_text)
                            .id_source(text_id)
                            .desired_width(FILTER_POPUP_WIDTH - 24.0)
                            .hint_text("Type to filter files..."),
                    );

                    if app.filter_popup_focus_pending {
                        text_res.request_focus();
                        set_cursor_to_end(ctx, text_id, filter_text.chars().count());
                        app.filter_popup_focus_pending = false;
                    }

                    if filter_text != app.state.filter.criteria.text {
                        crate::handlers::set_text_filter(app, filter_text);
                    }

                    if text_res.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        close_on_enter = true;
                    }
                });
            });
        });

    if close_on_enter {
        crate::handlers::close_filter_popup(app);
    }

    let should_handle_outside_click = app.show_filter_popup && !app.filter_popup_just_opened;
    if should_handle_outside_click && ctx.input(|i| i.pointer.any_pressed()) {
        let clicked_outside = ctx.pointer_interact_pos().is_some_and(|pos| !area_res.response.rect.contains(pos));
        if clicked_outside {
            crate::handlers::close_filter_popup(app);
        }
    }

    app.filter_popup_just_opened = false;
}

fn set_cursor_to_end(ctx: &egui::Context, text_id: egui::Id, char_count: usize) {
    if let Some(mut state) = egui::TextEdit::load_state(ctx, text_id) {
        let cursor = egui::text::CCursor::new(char_count);
        let range = egui::text_selection::CCursorRange::one(cursor);
        state.cursor.set_char_range(Some(range));
        state.store(ctx, text_id);
    }
}
