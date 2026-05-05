use eframe::egui;
use crate::app::ImageApp;
use crate::sync::pan_zoom::PanZoomSnapshot;

/// Result from split_layout::render, carrying the navigation action and the
/// screen rect of the *active* canvas pane (used for dialog backdrop positioning).
pub struct SplitLayoutResult {
    pub nav_action: Option<i32>,
    pub active_canvas_rect: egui::Rect,
}

pub fn render(
    app: &mut ImageApp,
    ctx: &egui::Context,
    pass_through_ui: &mut egui::Ui,
    allow_interaction: bool,
) -> SplitLayoutResult {
    let is_split = app.workspace.is_split();
    let immersive_topbar_visible = app.immersive_topbar_visible;

    if is_split {
        let mut left_nav = None;
        let mut right_nav = None;
        let active_index = app.workspace.active_view_index;
        let mut left_rect = egui::Rect::NOTHING;
        let mut right_rect = egui::Rect::NOTHING;
        let can_enable_sync = crate::sync::pan_zoom::can_enable_sync(app);
        if !app.split_pan_zoom_sync_user_disabled && !app.split_pan_zoom_sync_enabled && can_enable_sync {
            app.split_pan_zoom_sync_enabled = true;
        }
        let sync_enabled = app.split_pan_zoom_sync_enabled && can_enable_sync;

        // Split view
        pass_through_ui.columns(2, |columns| {
            // Left (cloned side) = index 1
            let left_pre = PanZoomSnapshot::from_state(&app.workspace.views[1]);
            let (nav, rect) = crate::ui::canvas::render(
                ctx,
                &mut columns[0],
                &mut app.workspace.views[1],
                app.settings.loop_playlist,
                app.settings.fit_all_images_to_window,
                app.settings.pixel_based_1_to_1,
                active_index == 1,
                true,
                immersive_topbar_visible,
                allow_interaction,
            );
            left_nav = nav;
            left_rect = rect;
            let left_post = PanZoomSnapshot::from_state(&app.workspace.views[1]);

            if sync_enabled && left_pre.differs_from(&left_post) {
                let (right_slice, _) = app.workspace.views.split_at_mut(1);
                left_post.apply_to(&mut right_slice[0]);
            }

            // Right (original side) = index 0
            let right_pre = PanZoomSnapshot::from_state(&app.workspace.views[0]);
            let (nav, rect) = crate::ui::canvas::render(
                ctx,
                &mut columns[1],
                &mut app.workspace.views[0],
                app.settings.loop_playlist,
                app.settings.fit_all_images_to_window,
                app.settings.pixel_based_1_to_1,
                active_index == 0,
                true,
                immersive_topbar_visible,
                allow_interaction,
            );
            right_nav = nav;
            right_rect = rect;
            let right_post = PanZoomSnapshot::from_state(&app.workspace.views[0]);

            if sync_enabled && right_pre.differs_from(&right_post) {
                let (_, left_slice) = app.workspace.views.split_at_mut(1);
                right_post.apply_to(&mut left_slice[0]);
            }
        });

        // Determine active canvas rect: index 0 -> right, index 1 -> left
        let active_canvas_rect = if active_index == 1 { left_rect } else { right_rect };

        // Handle focus switch: Some(0) means the pane was clicked just to gain focus.
        // Update the active index and invalidate the title cache, but don't navigate.
        if left_nav == Some(0) {
            app.workspace.active_view_index = 1;
            app.cached_title.clear();
            return SplitLayoutResult { nav_action: None, active_canvas_rect: left_rect };
        } else if right_nav == Some(0) {
            app.workspace.active_view_index = 0;
            app.cached_title.clear();
            return SplitLayoutResult { nav_action: None, active_canvas_rect: right_rect };
        }

        // Real navigation: set focus and propagate direction.
        if left_nav.is_some() {
            app.workspace.active_view_index = 1;
            app.cached_title.clear();
            return SplitLayoutResult { nav_action: left_nav, active_canvas_rect: left_rect };
        } else if right_nav.is_some() {
            app.workspace.active_view_index = 0;
            app.cached_title.clear();
            return SplitLayoutResult { nav_action: right_nav, active_canvas_rect: right_rect };
        }

        SplitLayoutResult { nav_action: None, active_canvas_rect }
    } else {
        let (nav_action, canvas_rect) = crate::ui::canvas::render(
            ctx,
            pass_through_ui,
            &mut app.workspace.views[0],
            app.settings.loop_playlist,
            app.settings.fit_all_images_to_window,
            app.settings.pixel_based_1_to_1,
            true, // Only one view, so it's always active
            false,
            false,
            allow_interaction,
        );
        SplitLayoutResult { nav_action, active_canvas_rect: canvas_rect }
    }
}
