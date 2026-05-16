use eframe::egui;
use crate::app::ImageApp;
use crate::playlist_grid::ThumbnailEntry;

/// Actions the grid can produce for the main update loop to handle.
pub enum PlaylistGridAction {
    None,
    /// User double-clicked an image — open it in canvas mode.
    OpenImage { path: std::path::PathBuf, index: usize },
    /// User clicked "Open File" in the empty-folder placeholder.
    OpenFile,
    /// User clicked "Open Folder" in the empty-folder placeholder.
    OpenFolder,
}

/// Render the playlist grid inside the given UI region.
///
/// Returns an action for the caller to dispatch.
pub fn render(
    app: &mut ImageApp,
    ctx: &egui::Context,
    ui: &mut egui::Ui,
) -> PlaylistGridAction {
    let grid = match app.workspace.playlist_grid.as_mut() {
        Some(g) => g,
        None => return PlaylistGridAction::None,
    };

    // Process any thumbnails that arrived since the last frame.
    grid.process_thumbnail_results(ctx);

    let active_view = &app.workspace.views[app.workspace.active_view_index];
    let playlist = &active_view.active_playlist;

    if playlist.is_empty() {
        return render_empty_folder(ui);
    }

    let settings = &grid.settings;
    let thumb_w = settings.thumbnail_width as f32;
    let min_spacing_x = settings.item_spacing_x;
    let spacing_y = settings.item_spacing_y;
    let label_h = settings.label_height;
    let padding_y = 12.0;

    let available_width = ui.available_width();
    let columns = ((available_width - min_spacing_x).max(0.0) / (thumb_w + min_spacing_x))
        .floor()
        .max(1.0) as usize;

    // Build rows.
    let total_items = playlist.len();
    let row_count = (total_items + columns - 1) / columns;

    // Pre-calculate per-item thumbnail display sizes.
    // For each item, determine the height it would take at `thumb_w` width
    // while preserving aspect ratio.
    let mut item_display_heights: Vec<f32> = Vec::with_capacity(total_items);
    for path in playlist.iter() {
        let h = match grid.thumbnail_cache.get(path) {
            Some(ThumbnailEntry::Ready { width, height, .. }) if *width > 0 => {
                let aspect = *height as f32 / *width as f32;
                thumb_w * aspect
            }
            _ => thumb_w, // Square placeholder for loading/error/unknown.
        };
        item_display_heights.push(h);
    }

    let mut action = PlaylistGridAction::None;

    // Handle scroll-to-index request.
    let scroll_to_idx = grid.scroll_to_index.take();

    let scroll_id = egui::Id::new("playlist_grid_scroll");

    let mut scroll_area = egui::ScrollArea::vertical()
        .id_salt(scroll_id)
        .auto_shrink([false, false]);

    // If we need to scroll to a specific index, calculate the approximate Y
    // and set the scroll offset.  This is an approximation — we use the
    // average row height since we can't know exact row heights before layout.
    if let Some(target_idx) = scroll_to_idx {
        let target_row = target_idx / columns;
        // Estimate each row's height as thumb_w + label_h + spacing_y.
        let est_row_h = thumb_w + label_h + spacing_y;
        let target_y = padding_y + target_row as f32 * est_row_h;
        // Centre the target in the viewport.
        let viewport_h = ui.available_height();
        let scroll_y = (target_y - viewport_h / 2.0 + est_row_h / 2.0).max(0.0);
        scroll_area = scroll_area.vertical_scroll_offset(scroll_y);
    }

    let scroll_output = scroll_area.show(ui, |ui| {
        // Determine the visible Y range for lazy-loading.
        let clip_rect = ui.clip_rect();

        // Track which items are (approximately) visible.
        let mut visible_paths: Vec<std::path::PathBuf> = Vec::new();

        ui.add_space(padding_y);

        for row in 0..row_count {
            let row_start = row * columns;
            let row_end = (row_start + columns).min(total_items);
            let items_in_row = row_end.saturating_sub(row_start);

            // Compute row height = tallest item + label.
            let row_thumb_h = item_display_heights[row_start..row_end]
                .iter()
                .cloned()
                .fold(0.0f32, f32::max);
            let row_h = row_thumb_h + label_h;

            // Allocate a rect for this row.
            let (row_rect, _) = ui.allocate_exact_size(
                egui::vec2(available_width, row_h + spacing_y),
                egui::Sense::hover(),
            );
            let row_content_rect = egui::Rect::from_min_size(
                row_rect.min,
                egui::vec2(available_width, row_h),
            );

            // Skip rendering if the row is outside the visible clip region.
            let row_visible = row_rect.max.y >= clip_rect.min.y
                && row_rect.min.y <= clip_rect.max.y;

            if !row_visible {
                continue;
            }

            let gap_x = ((available_width - columns as f32 * thumb_w)
                / (columns as f32 + 1.0))
                .max(0.0);
            let highlight_inset_x = gap_x * 0.25;
            let highlight_pad_top = 8.0;
            let highlight_pad_bottom = 0.0;

            // Render each item in this row.
            for col in 0..items_in_row {
                let item_idx = row_start + col;

                let path = &playlist[item_idx];
                visible_paths.push(path.clone());

                let item_thumb_h = item_display_heights[item_idx];

                // Position this item within the row.
                let x = row_content_rect.min.x + gap_x + col as f32 * (thumb_w + gap_x);
                let cell_rect = egui::Rect::from_min_size(
                    egui::pos2(x - gap_x * 0.5, row_content_rect.min.y),
                    egui::vec2(thumb_w + gap_x, row_content_rect.height()),
                );
                let highlight_rect = egui::Rect::from_min_max(
                    egui::pos2(
                        cell_rect.min.x + highlight_inset_x,
                        cell_rect.min.y - highlight_pad_top,
                    ),
                    egui::pos2(
                        cell_rect.max.x - highlight_inset_x,
                        cell_rect.max.y + highlight_pad_bottom,
                    ),
                );
                // Vertically centre the thumbnail within the row's thumbnail area.
                let thumb_y_offset = (row_thumb_h - item_thumb_h) / 2.0;
                let thumb_rect = egui::Rect::from_min_size(
                    egui::pos2(x, row_content_rect.min.y + thumb_y_offset),
                    egui::vec2(thumb_w, item_thumb_h),
                );
                let label_rect = egui::Rect::from_min_size(
                    egui::pos2(x, row_content_rect.min.y + row_thumb_h),
                    egui::vec2(thumb_w, label_h),
                );
                let full_item_rect = egui::Rect::from_min_max(
                    thumb_rect.min,
                    label_rect.max,
                );

                // Interaction sensing.
                let item_id = egui::Id::new(("grid_item", item_idx));
                let response = ui.interact(full_item_rect, item_id, egui::Sense::click());

                // Selection highlight.
                let is_selected = grid.selection.is_selected(item_idx);
                if is_selected {
                    let highlight_color =
                        egui::Color32::from_rgba_unmultiplied(60, 120, 215, 60);
                    ui.painter().rect_filled(highlight_rect, 4.0, highlight_color);
                    let border_color = egui::Color32::from_rgb(60, 120, 215);
                    ui.painter().rect_stroke(
                        highlight_rect,
                        4.0,
                        egui::Stroke::new(1.5, border_color),
                        egui::StrokeKind::Inside,
                    );
                }

                // Hover highlight.
                if response.hovered() && !is_selected {
                    let hover_color = egui::Color32::from_white_alpha(15);
                    ui.painter().rect_filled(highlight_rect, 4.0, hover_color);
                }

                // Draw thumbnail content.
                match grid.thumbnail_cache.get(path) {
                    Some(ThumbnailEntry::Ready { texture, .. }) => {
                        let uv = egui::Rect::from_min_max(
                            egui::pos2(0.0, 0.0),
                            egui::pos2(1.0, 1.0),
                        );
                        ui.painter().image(texture.id(), thumb_rect, uv, egui::Color32::WHITE);
                    }
                    Some(ThumbnailEntry::Error(_)) => {
                        // Error icon centred in the thumbnail area.
                        let icon_color = ui.visuals().error_fg_color;
                        ui.painter().text(
                            thumb_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            egui_phosphor::regular::IMAGE_BROKEN,
                            egui::FontId::proportional(32.0),
                            icon_color,
                        );
                    }
                    Some(ThumbnailEntry::Loading) | None => {
                        // Spinner placeholder.
                        let spinner_rect = egui::Rect::from_center_size(
                            thumb_rect.center(),
                            egui::vec2(20.0, 20.0),
                        );
                        ui.put(spinner_rect, egui::Spinner::new().size(16.0));
                    }
                }

                // Draw filename label.
                let file_name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "???".to_string());

                let label_galley = ui.painter().layout(
                    file_name,
                    egui::FontId::proportional(11.0),
                    ui.visuals().text_color(),
                    thumb_w,
                );
                let text_pos = egui::pos2(
                    label_rect.center().x - label_galley.size().x / 2.0,
                    label_rect.min.y + 2.0,
                );
                ui.painter().galley(text_pos, label_galley, egui::Color32::TRANSPARENT);

                // Handle click interactions.
                if response.double_clicked() {
                    action = PlaylistGridAction::OpenImage {
                        path: path.clone(),
                        index: item_idx,
                    };
                } else if response.clicked() {
                    let modifiers = ui.input(|i| i.modifiers);
                    grid.selection.handle_click(
                        item_idx,
                        modifiers.ctrl || modifiers.command,
                        modifiers.shift,
                        total_items,
                    );
                }

                if response.hovered() {
                    ctx.set_cursor_icon(egui::CursorIcon::Default);
                }
            }
        }

        ui.add_space(padding_y);

        // Lazy-load: request thumbnails for visible items.
        grid.request_thumbnails_for_paths(&visible_paths);
    });

    // Suppress unused variable warning.
    let _ = scroll_output;

    action
}

/// Render the empty-folder placeholder (no images found).
fn render_empty_folder(ui: &mut egui::Ui) -> PlaylistGridAction {
    let mut action = PlaylistGridAction::None;

    let area_rect = ui.max_rect();
    let mut group_ui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(area_rect)
            .layout(egui::Layout::top_down(egui::Align::Center)),
    );

    let content_height = 80.0;
    let top_padding = ((area_rect.height() - content_height) / 2.0).max(0.0);
    group_ui.add_space(top_padding);

    group_ui.add(
        egui::Label::new("No images found.\nDrag and drop a file or folder here.")
            .selectable(false),
    );
    group_ui.add_space(8.0);

    let open_file_btn = egui::Button::new(
        egui::RichText::new(format!(
            "{}  Open File",
            egui_phosphor::regular::FILE_IMAGE
        ))
        .size(14.0),
    )
    .min_size(egui::vec2(120.0, 30.0));

    if group_ui.add(open_file_btn).clicked() {
        action = PlaylistGridAction::OpenFile;
    }

    group_ui.add_space(4.0);

    let open_folder_btn = egui::Button::new(
        egui::RichText::new(format!(
            "{}  Open Folder",
            egui_phosphor::regular::FOLDER_OPEN
        ))
        .size(14.0),
    )
    .min_size(egui::vec2(120.0, 30.0));

    if group_ui.add(open_folder_btn).clicked() {
        action = PlaylistGridAction::OpenFolder;
    }

    action
}
