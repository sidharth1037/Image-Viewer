pub mod items;

use eframe::egui;

// ── Layout constants ────────────────────────────────────────────────────────

const MENU_WIDTH: f32 = 220.0;
const ROW_HEIGHT: f32 = 28.0;
const SEPARATOR_HEIGHT: f32 = 9.0;
const MENU_PADDING: f32 = 4.0;
const ICON_COL_WIDTH: f32 = 28.0;
const H_PADDING: f32 = 8.0;
const CORNER_RADIUS: f32 = 8.0;
const ICON_FONT_SIZE: f32 = 16.0;
const SHORTCUT_RIGHT_PAD: f32 = 10.0;

// ── Public types ────────────────────────────────────────────────────────────

/// A single clickable row in the context menu.
pub struct ContextMenuItem {
    pub icon: &'static str,
    pub label: &'static str,
    pub shortcut_hint: Option<&'static str>,
    pub enabled: bool,
    pub id: &'static str,
}

/// An entry in the context menu — either an item or a visual separator.
pub enum ContextMenuEntry {
    Item(ContextMenuItem),
    Separator,
}

/// Returned when the user clicks an enabled item.
pub struct ContextMenuAction {
    pub id: &'static str,
}

/// Persistent state tracked across frames.
pub struct ContextMenuState {
    pub open: bool,
    pub position: egui::Pos2,
    pub entries: Vec<ContextMenuEntry>,
}

impl Default for ContextMenuState {
    fn default() -> Self {
        Self {
            open: false,
            position: egui::Pos2::ZERO,
            entries: Vec::new(),
        }
    }
}

impl ContextMenuState {
    /// Open the context menu at the given cursor position with the provided entries.
    pub fn open(&mut self, pos: egui::Pos2, entries: Vec<ContextMenuEntry>) {
        self.open = true;
        self.position = pos;
        self.entries = entries;
    }

    /// Close the context menu.
    pub fn close(&mut self) {
        self.open = false;
        self.entries.clear();
    }
}

// ── Rendering ───────────────────────────────────────────────────────────────

/// Render the context menu overlay. Returns an action if the user clicked an
/// enabled item, `None` otherwise.
pub fn render(state: &mut ContextMenuState, ctx: &egui::Context) -> Option<ContextMenuAction> {
    if !state.open {
        return None;
    }

    // Compute total menu height from entries.
    let menu_height = compute_menu_height(&state.entries);
    let menu_size = egui::vec2(MENU_WIDTH, menu_height);

    // Edge-aware positioning: flip if the menu would overflow the screen.
    let screen = ctx.content_rect();
    let pos = edge_aware_position(state.position, menu_size, screen);

    let mut clicked_action: Option<ContextMenuAction> = None;

    let area_res = egui::Area::new(egui::Id::new("context_menu_overlay"))
        .fixed_pos(pos)
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            let frame = egui::Frame::menu(ui.style())
                .corner_radius(CORNER_RADIUS)
                .inner_margin(egui::Margin::symmetric(0, MENU_PADDING as i8))
                .shadow(egui::Shadow {
                    offset: [0, 2],
                    blur: 12,
                    spread: 0,
                    color: egui::Color32::from_black_alpha(60),
                });

            frame.show(ui, |ui| {
                ui.set_width(MENU_WIDTH);

                for entry in &state.entries {
                    match entry {
                        ContextMenuEntry::Separator => {
                            render_separator(ui);
                        }
                        ContextMenuEntry::Item(item) => {
                            if let Some(action) = render_item(ui, item) {
                                clicked_action = Some(action);
                            }
                        }
                    }
                }
            });
        });

    // Dismiss on click outside the menu.
    if ctx.input(|i| i.pointer.any_pressed()) {
        let clicked_outside = ctx.pointer_interact_pos().is_some_and(|click_pos| {
            !area_res.response.rect.contains(click_pos)
        });
        if clicked_outside {
            state.close();
            return None;
        }
    }

    // Close after an action was selected.
    if clicked_action.is_some() {
        state.close();
    }

    clicked_action
}

// ── Private helpers ─────────────────────────────────────────────────────────

fn compute_menu_height(entries: &[ContextMenuEntry]) -> f32 {
    let mut h = MENU_PADDING * 2.0;
    for entry in entries {
        match entry {
            ContextMenuEntry::Separator => h += SEPARATOR_HEIGHT,
            ContextMenuEntry::Item(_) => h += ROW_HEIGHT,
        }
    }
    h
}

fn edge_aware_position(cursor: egui::Pos2, size: egui::Vec2, screen: egui::Rect) -> egui::Pos2 {
    let mut x = cursor.x;
    let mut y = cursor.y;

    // Flip horizontally if overflowing to the right.
    if x + size.x > screen.max.x {
        x = cursor.x - size.x;
    }
    // Flip vertically if overflowing at the bottom.
    if y + size.y > screen.max.y {
        y = cursor.y - size.y;
    }

    // Clamp to ensure we never go off-screen.
    x = x.clamp(screen.min.x, (screen.max.x - size.x).max(screen.min.x));
    y = y.clamp(screen.min.y, (screen.max.y - size.y).max(screen.min.y));

    egui::pos2(x, y)
}

fn render_separator(ui: &mut egui::Ui) {
    let sep_size = egui::vec2(ui.available_width(), SEPARATOR_HEIGHT);
    let (rect, _) = ui.allocate_exact_size(sep_size, egui::Sense::hover());

    if ui.is_rect_visible(rect) {
        let stroke_color = ui.visuals().widgets.noninteractive.bg_stroke.color;
        let y = rect.center().y;
        let inset = H_PADDING;
        ui.painter().hline(
            egui::Rangef::new(rect.min.x + inset, rect.max.x - inset),
            y,
            egui::Stroke::new(1.0, stroke_color),
        );
    }
}

fn render_item(ui: &mut egui::Ui, item: &ContextMenuItem) -> Option<ContextMenuAction> {
    let row_size = egui::vec2(ui.available_width(), ROW_HEIGHT);
    let (rect, response) = ui.allocate_exact_size(row_size, egui::Sense::click());

    if ui.is_rect_visible(rect) {
        let visuals = &ui.style().visuals;

        let (bg_fill, fg_color, shortcut_color) = if !item.enabled {
            // Disabled: dimmed text, no hover.
            (
                egui::Color32::TRANSPARENT,
                visuals.text_color().gamma_multiply(0.35),
                visuals.text_color().gamma_multiply(0.25),
            )
        } else if response.hovered() {
            // Hovered: highlight background.
            let widget = &visuals.widgets.hovered;
            (
                widget.bg_fill,
                widget.fg_stroke.color,
                visuals.weak_text_color(),
            )
        } else {
            // Normal: no background.
            let widget = &visuals.widgets.inactive;
            (
                egui::Color32::TRANSPARENT,
                widget.fg_stroke.color,
                visuals.weak_text_color(),
            )
        };

        // Background fill (with rounded corners for hover).
        if bg_fill != egui::Color32::TRANSPARENT {
            let inset_rect = rect.shrink2(egui::vec2(4.0, 1.0));
            ui.painter().rect_filled(inset_rect, 4.0, bg_fill);
        }

        // Icon column.
        let icon_x = rect.min.x + H_PADDING + ICON_COL_WIDTH / 2.0;
        ui.painter().text(
            egui::pos2(icon_x, rect.center().y),
            egui::Align2::CENTER_CENTER,
            item.icon,
            egui::FontId::proportional(ICON_FONT_SIZE),
            fg_color,
        );

        // Label.
        let label_x = rect.min.x + H_PADDING + ICON_COL_WIDTH + 4.0;
        ui.painter().text(
            egui::pos2(label_x, rect.center().y),
            egui::Align2::LEFT_CENTER,
            item.label,
            egui::TextStyle::Button.resolve(ui.style()),
            fg_color,
        );

        // Shortcut hint (right-aligned).
        if let Some(shortcut) = item.shortcut_hint {
            let shortcut_x = rect.max.x - SHORTCUT_RIGHT_PAD;
            ui.painter().text(
                egui::pos2(shortcut_x, rect.center().y),
                egui::Align2::RIGHT_CENTER,
                shortcut,
                egui::FontId::proportional(11.0),
                shortcut_color,
            );
        }
    }

    if item.enabled && response.clicked() {
        Some(ContextMenuAction { id: item.id })
    } else {
        None
    }
}
