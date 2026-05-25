use eframe::egui;
use crate::app::ImageApp;
use crate::playlist_grid::ThumbnailEntry;

const TITLE_BAR_HEIGHT: f32 = 32.0;
const GROUP_HEADER_HEIGHT: f32 = 24.0;
const ROW_PADDING_Y: f32 = 8.0;

/// Actions the duplicate view can produce for the main update loop to handle.
pub enum DuplicateViewAction {
    None,
    /// User double-clicked a thumbnail — open it in canvas mode with the row
    /// as the playlist.
    OpenImage {
        group_index: usize,
        path: std::path::PathBuf,
        index_in_group: usize,
    },
}

/// Render the duplicate finder view in the given UI rect.
///
/// The layout is:
///   1. Title bar ("Duplicate Files" + stats)
///   2. Vertically scrollable area containing duplicate groups
///   3. Each group: header + horizontally scrollable row of thumbnails
pub fn render(
    app: &mut ImageApp,
    ctx: &egui::Context,
    ui: &mut egui::Ui,
    content_rect: egui::Rect,
) -> DuplicateViewAction {
    let mut action = DuplicateViewAction::None;

    // ── Title bar ───────────────────────────────────────────────────────
    let title_rect = egui::Rect::from_min_size(
        content_rect.min,
        egui::vec2(content_rect.width(), TITLE_BAR_HEIGHT),
    );
    render_title_bar(app, ui, title_rect);

    // ── Content area (below title bar) ──────────────────────────────────
    let body_rect = egui::Rect::from_min_max(
        egui::pos2(content_rect.min.x, content_rect.min.y + TITLE_BAR_HEIGHT),
        content_rect.max,
    );

    let dup_state = match app.workspace.duplicate_finder.as_mut() {
        Some(s) => s,
        None => return DuplicateViewAction::None,
    };

    if dup_state.scanning {
        // Show scanning indicator.
        ui.scope_builder(egui::UiBuilder::new().max_rect(body_rect), |ui| {
            let area_rect = ui.max_rect();
            let mut group_ui = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(area_rect)
                    .layout(egui::Layout::top_down(egui::Align::Center)),
            );
            let top_padding = ((area_rect.height() - 64.0) / 2.0).max(0.0);
            group_ui.add_space(top_padding);
            group_ui.add(egui::Spinner::new().size(20.0));
            group_ui.add_space(8.0);
            group_ui.add(egui::Label::new("Scanning for duplicates...").selectable(false));
        });
        ctx.request_repaint();
        return DuplicateViewAction::None;
    }

    if dup_state.groups.is_empty() {
        // No duplicates found.
        ui.scope_builder(egui::UiBuilder::new().max_rect(body_rect), |ui| {
            let area_rect = ui.max_rect();
            let mut group_ui = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(area_rect)
                    .layout(egui::Layout::top_down(egui::Align::Center)),
            );
            let top_padding = ((area_rect.height() - 60.0) / 2.0).max(0.0);
            group_ui.add_space(top_padding);
            group_ui.add(
                egui::Label::new("No duplicate files found.")
                    .selectable(false),
            );
        });
        return DuplicateViewAction::None;
    }

    // ── Render duplicate groups ─────────────────────────────────────────
    let grid = match app.workspace.playlist_grid.as_mut() {
        Some(g) => g,
        None => return DuplicateViewAction::None,
    };

    // Process any thumbnails that arrived since the last frame.
    grid.process_thumbnail_results(ctx);

    let thumb_w = grid.settings.thumbnail_width as f32;
    let max_height_ratio = grid.settings.max_height_ratio;
    let max_thumb_h = thumb_w * max_height_ratio;
    let label_font = egui::FontId::proportional(11.0);
    let label_line_height = ui.painter().fonts_mut(|f| f.row_height(&label_font));
    let label_h = grid.settings.label_height.max(label_line_height * 2.0 + 4.0);
    let row_height = max_thumb_h + label_h;
    let spacing_x = grid.settings.item_spacing_x;

    // We need to re-borrow dup_state after borrowing grid; this works because
    // they are on different fields of workspace.
    let dup_state = app.workspace.duplicate_finder.as_mut().unwrap();
    let group_count = dup_state.groups.len();

    // Collect all visible paths for lazy thumbnail loading.
    let mut all_visible_paths: Vec<std::path::PathBuf> = Vec::new();

    ui.scope_builder(egui::UiBuilder::new().max_rect(body_rect), |ui| {
        egui::ScrollArea::vertical()
            .id_salt("duplicate_view_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add_space(ROW_PADDING_Y);

                for group_idx in 0..group_count {
                    let group = &dup_state.groups[group_idx];
                    let file_count = group.paths.len();
                    let paths_snapshot: Vec<std::path::PathBuf> = group.paths.clone();

                    // ── Group header ──
                    let (header_rect, _) = ui.allocate_exact_size(
                        egui::vec2(ui.available_width(), GROUP_HEADER_HEIGHT),
                        egui::Sense::hover(),
                    );

                    let header_text = format!("Group {} — {} files", group_idx + 1, file_count);
                    let header_color = ui.visuals().weak_text_color();
                    ui.painter().text(
                        egui::pos2(header_rect.min.x + 8.0, header_rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        &header_text,
                        egui::FontId::proportional(12.0),
                        header_color,
                    );

                    // Separator line below header.
                    let sep_stroke = egui::Stroke::new(
                        1.0,
                        ui.visuals().widgets.noninteractive.bg_stroke.color,
                    );
                    ui.painter().hline(
                        header_rect.x_range(),
                        header_rect.bottom(),
                        sep_stroke,
                    );

                    // ── Horizontally scrollable thumbnail row ──
                    let scroll_id = egui::Id::new(("dup_row_scroll", group_idx));
                    let (row_rect, _) = ui.allocate_exact_size(
                        egui::vec2(ui.available_width(), row_height + ROW_PADDING_Y * 2.0),
                        egui::Sense::hover(),
                    );

                    let inner_row_rect = egui::Rect::from_min_max(
                        egui::pos2(row_rect.min.x, row_rect.min.y + ROW_PADDING_Y),
                        egui::pos2(row_rect.max.x, row_rect.max.y - ROW_PADDING_Y),
                    );

                    // Only render if the row is visible.
                    let clip_rect = ui.clip_rect();
                    let row_visible = row_rect.max.y >= clip_rect.min.y
                        && row_rect.min.y <= clip_rect.max.y;

                    if !row_visible {
                        continue;
                    }

                    // Render thumbnails in horizontal scroll.
                    let mut row_ui = ui.new_child(
                        egui::UiBuilder::new().max_rect(inner_row_rect),
                    );

                    egui::ScrollArea::horizontal()
                        .id_salt(scroll_id)
                        .auto_shrink([false, false])
                        .show(&mut row_ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = spacing_x;

                                for item_idx in 0..paths_snapshot.len() {
                                    let path = &paths_snapshot[item_idx];
                                    all_visible_paths.push(path.clone());

                                    let aspect = match grid.thumbnail_cache.get(path) {
                                        Some(ThumbnailEntry::Ready { width, height, .. }) if *width > 0 => {
                                            *height as f32 / *width as f32
                                        }
                                        _ => 1.0,
                                    };
                                    let (item_thumb_w, item_thumb_h) = if aspect > max_height_ratio {
                                        (max_thumb_h / aspect, max_thumb_h)
                                    } else {
                                        (thumb_w, thumb_w * aspect)
                                    };

                                    let cell_size = egui::vec2(thumb_w, row_height);
                                    let (cell_rect, response) =
                                        ui.allocate_exact_size(cell_size, egui::Sense::click());

                                    let thumb_y_offset = (max_thumb_h - item_thumb_h) / 2.0;
                                    let thumb_x_offset = (thumb_w - item_thumb_w) / 2.0;
                                    let thumb_rect = egui::Rect::from_min_size(
                                        egui::pos2(
                                            cell_rect.min.x + thumb_x_offset,
                                            cell_rect.min.y + thumb_y_offset,
                                        ),
                                        egui::vec2(item_thumb_w, item_thumb_h),
                                    );
                                    let label_rect = egui::Rect::from_min_size(
                                        egui::pos2(cell_rect.min.x, cell_rect.min.y + max_thumb_h),
                                        egui::vec2(thumb_w, label_h),
                                    );

                                    // Selection highlight.
                                    let is_selected = dup_state.groups[group_idx]
                                        .selection
                                        .is_selected(item_idx);

                                    if is_selected {
                                        let highlight_color =
                                            egui::Color32::from_rgba_unmultiplied(60, 120, 215, 60);
                                        ui.painter().rect_filled(cell_rect, 4.0, highlight_color);
                                        let border_color = egui::Color32::from_rgb(60, 120, 215);
                                        ui.painter().rect_stroke(
                                            cell_rect,
                                            4.0,
                                            egui::Stroke::new(1.5, border_color),
                                            egui::StrokeKind::Inside,
                                        );
                                    }

                                    // Hover highlight.
                                    if response.hovered() && !is_selected {
                                        let hover_color = egui::Color32::from_white_alpha(15);
                                        ui.painter().rect_filled(cell_rect, 4.0, hover_color);
                                    }

                                    // Draw thumbnail.
                                    match grid.thumbnail_cache.get(path) {
                                        Some(ThumbnailEntry::Ready { texture, .. }) => {
                                            let uv = egui::Rect::from_min_max(
                                                egui::pos2(0.0, 0.0),
                                                egui::pos2(1.0, 1.0),
                                            );
                                            ui.painter().image(
                                                texture.id(),
                                                thumb_rect,
                                                uv,
                                                egui::Color32::WHITE,
                                             );
                                        }
                                        Some(ThumbnailEntry::Error(_)) => {
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

                                    let text_color = ui.visuals().text_color();
                                    let mut job = egui::text::LayoutJob::simple(
                                        file_name,
                                        label_font.clone(),
                                        text_color,
                                        thumb_w,
                                    );
                                    job.wrap.max_width = thumb_w;
                                    job.wrap.max_rows = 2;
                                    job.wrap.break_anywhere = true;
                                    job.wrap.overflow_character = None;
                                    let label_galley =
                                        ui.painter().fonts_mut(|f| f.layout_job(job));

                                    let text_pos = egui::pos2(
                                        label_rect.center().x - label_galley.size().x / 2.0,
                                        label_rect.min.y + 2.0,
                                    );
                                    ui.painter().galley(
                                        text_pos,
                                        label_galley,
                                        egui::Color32::TRANSPARENT,
                                    );

                                    // Handle interactions.
                                    if response.double_clicked() {
                                        action = DuplicateViewAction::OpenImage {
                                            group_index: group_idx,
                                            path: path.clone(),
                                            index_in_group: item_idx,
                                        };
                                    } else if response.clicked() {
                                        let modifiers = ui.input(|i| i.modifiers);
                                        let total_items = paths_snapshot.len();
                                        dup_state.groups[group_idx]
                                            .selection
                                            .handle_click(
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
                            });
                        });

                    // Separator between groups.
                    ui.add_space(ROW_PADDING_Y);
                }

                ui.add_space(ROW_PADDING_Y);
            });
    });

    // Lazy-load thumbnails for visible items.
    if let Some(grid) = app.workspace.playlist_grid.as_mut() {
        grid.request_thumbnails_for_paths(&all_visible_paths);
    }

    action
}

/// Render the title bar strip at the top of the duplicate finder view.
fn render_title_bar(app: &ImageApp, ui: &mut egui::Ui, rect: egui::Rect) {
    let bg = ui.visuals().window_fill();
    ui.painter().rect_filled(rect, 0.0, bg);

    // Bottom separator.
    let sep_stroke = egui::Stroke::new(
        1.0,
        ui.visuals().widgets.noninteractive.bg_stroke.color,
    );
    ui.painter().hline(rect.x_range(), rect.bottom(), sep_stroke);

    // Title text.
    let title = "Duplicate Files";
    let title_color = ui.visuals().strong_text_color();
    ui.painter().text(
        egui::pos2(rect.min.x + 12.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        title,
        egui::FontId::proportional(13.0),
        title_color,
    );

    // Stats on the right.
    if let Some(dup_state) = app.workspace.duplicate_finder.as_ref() {
        let total_groups = dup_state.groups.len();
        let total_files: usize = dup_state.groups.iter().map(|g| g.paths.len()).sum();
        let stats = format!("{} groups, {} files", total_groups, total_files);
        let stats_color = ui.visuals().weak_text_color();
        ui.painter().text(
            egui::pos2(rect.max.x - 12.0, rect.center().y),
            egui::Align2::RIGHT_CENTER,
            &stats,
            egui::FontId::proportional(12.0),
            stats_color,
        );
    }
}
