use eframe::egui;
use crate::app::ImageApp;

pub fn render(
    app: &mut ImageApp,
    ctx: &egui::Context,
    pass_through_ui: &mut egui::Ui,
    allow_interaction: bool,
) -> Option<i32> {
    let is_split = app.workspace.is_split();
    let immersive_topbar_visible = app.immersive_topbar_visible;

    if is_split {
        let mut left_nav = None;
        let mut right_nav = None;
        let active_index = app.workspace.active_view_index;

        // Split view
        pass_through_ui.columns(2, |columns| {
            // Left (cloned side) = index 1
            left_nav = crate::ui::canvas::render(
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

            // Right (original side) = index 0
            right_nav = crate::ui::canvas::render(
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
        });

        // Handle focus switch: Some(0) means the pane was clicked just to gain focus.
        // Update the active index and invalidate the title cache, but don't navigate.
        if left_nav == Some(0) {
            app.workspace.active_view_index = 1;
            app.cached_title.clear();
            return None;
        } else if right_nav == Some(0) {
            app.workspace.active_view_index = 0;
            app.cached_title.clear();
            return None;
        }

        // Real navigation: set focus and propagate direction.
        if left_nav.is_some() {
            app.workspace.active_view_index = 1;
            app.cached_title.clear();
            return left_nav;
        } else if right_nav.is_some() {
            app.workspace.active_view_index = 0;
            app.cached_title.clear();
            return right_nav;
        }

        None
    } else {
        crate::ui::canvas::render(
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
        )
    }
}
