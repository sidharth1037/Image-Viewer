use crate::app::ImageApp;
use super::{ContextMenuEntry, ContextMenuItem};
use egui_phosphor::regular as icons;

/// Build context menu entries for the **Canvas** view.
pub fn canvas_items(app: &ImageApp) -> Vec<ContextMenuEntry> {
    let view = app.workspace.active_view();
    let has_file = view.current_file_path.is_some();
    let has_adjustments = view.adjustments.has_adjustments()
        || view.rotation_quarter_turns != 0;
    let has_image = !view.frames.is_empty();

    let mut entries = Vec::new();

    if has_image {
        entries.push(ContextMenuEntry::Item(ContextMenuItem {
            icon: icons::ARROW_CLOCKWISE,
            label: "Rotate Clockwise".into(),
            shortcut_hint: Some("Shift+R"),
            enabled: has_image,
            id: "rotate_clockwise",
        }));

        entries.push(ContextMenuEntry::Item(ContextMenuItem {
            icon: icons::FLOPPY_DISK,
            label: "Save with Adjustments".into(),
            shortcut_hint: Some("Ctrl+Shift+S"),
            enabled: has_adjustments,
            id: "save_adjustments",
        }));

        entries.push(ContextMenuEntry::Separator);
    }

    entries.push(ContextMenuEntry::Item(ContextMenuItem {
        icon: icons::FOLDER_OPEN,
        label: "Reveal in Explorer".into(),
        shortcut_hint: Some("Ctrl+Shift+E"),
        enabled: has_file,
        id: "reveal_in_explorer",
    }));

    entries.push(ContextMenuEntry::Item(ContextMenuItem {
        icon: icons::CLIPBOARD,
        label: "Copy Path".into(),
        shortcut_hint: None,
        enabled: has_file,
        id: "copy_path",
    }));

    entries.push(ContextMenuEntry::Item(ContextMenuItem {
        icon: icons::COPY,
        label: "Copy Image".into(),
        shortcut_hint: Some("Ctrl+C"),
        enabled: has_file,
        id: "copy_files",
    }));

    entries.push(ContextMenuEntry::Separator);

    entries.push(ContextMenuEntry::Item(ContextMenuItem {
        icon: icons::TRASH,
        label: "Delete Permanently".into(),
        shortcut_hint: Some("Shift+Del"),
        enabled: has_file,
        id: "delete",
    }));

    entries
}

/// Build context menu entries for the **PlaylistGrid** view.
pub fn playlist_items(app: &ImageApp) -> Vec<ContextMenuEntry> {
    let view = app.workspace.active_view();
    let has_items = !view.active_playlist.is_empty();
    let selection_count = app
        .workspace
        .playlist_grid
        .as_ref()
        .map(|grid| grid.selection.selected.len())
        .unwrap_or(0);
    let has_selection = selection_count > 0;

    let mut entries = Vec::new();

    entries.push(ContextMenuEntry::Item(ContextMenuItem {
        icon: icons::IMAGE,
        label: "Open Image".into(),
        shortcut_hint: None,
        enabled: has_selection,
        id: "open_image",
    }));

    entries.push(ContextMenuEntry::Item(ContextMenuItem {
        icon: icons::FOLDER_OPEN,
        label: "Reveal in Explorer".into(),
        shortcut_hint: Some("Ctrl+Shift+E"),
        enabled: has_selection,
        id: "reveal_in_explorer",
    }));

    entries.push(ContextMenuEntry::Item(ContextMenuItem {
        icon: icons::CLIPBOARD,
        label: "Copy Path".into(),
        shortcut_hint: None,
        enabled: has_selection,
        id: "copy_path",
    }));

    entries.push(ContextMenuEntry::Item(ContextMenuItem {
        icon: icons::COPY,
        label: copy_files_label(selection_count),
        shortcut_hint: Some("Ctrl+C"),
        enabled: has_selection,
        id: "copy_files",
    }));

    entries.push(ContextMenuEntry::Separator);

    entries.push(ContextMenuEntry::Item(ContextMenuItem {
        icon: icons::CHECKS,
        label: "Select All".into(),
        shortcut_hint: Some("Ctrl+A"),
        enabled: has_items,
        id: "select_all",
    }));

    entries.push(ContextMenuEntry::Separator);

    entries.push(ContextMenuEntry::Item(ContextMenuItem {
        icon: icons::TRASH,
        label: "Delete Permanently".into(),
        shortcut_hint: Some("Shift+Del"),
        enabled: has_selection,
        id: "delete",
    }));

    entries
}

/// Build context menu entries for the **DuplicateFinder** view.
pub fn duplicate_items(app: &ImageApp) -> Vec<ContextMenuEntry> {
    let selection_count: usize = app
        .workspace
        .duplicate_finder
        .as_ref()
        .map(|dup| {
            dup.active_scan()
                .groups
                .iter()
                .map(|g| g.selection.selected.len())
                .sum()
        })
        .unwrap_or(0);
    let has_selection = selection_count > 0;

    let mut entries = Vec::new();

    entries.push(ContextMenuEntry::Item(ContextMenuItem {
        icon: icons::IMAGE,
        label: "Open Image".into(),
        shortcut_hint: None,
        enabled: has_selection,
        id: "open_image",
    }));

    entries.push(ContextMenuEntry::Item(ContextMenuItem {
        icon: icons::FOLDER_OPEN,
        label: "Reveal in Explorer".into(),
        shortcut_hint: Some("Ctrl+Shift+E"),
        enabled: has_selection,
        id: "reveal_in_explorer",
    }));

    entries.push(ContextMenuEntry::Item(ContextMenuItem {
        icon: icons::CLIPBOARD,
        label: "Copy Path".into(),
        shortcut_hint: None,
        enabled: has_selection,
        id: "copy_path",
    }));

    entries.push(ContextMenuEntry::Item(ContextMenuItem {
        icon: icons::COPY,
        label: copy_files_label(selection_count),
        shortcut_hint: Some("Ctrl+C"),
        enabled: has_selection,
        id: "copy_files",
    }));

    entries.push(ContextMenuEntry::Separator);

    entries.push(ContextMenuEntry::Item(ContextMenuItem {
        icon: icons::TRASH,
        label: "Delete Permanently".into(),
        shortcut_hint: Some("Shift+Del"),
        enabled: has_selection,
        id: "delete",
    }));

    entries
}

/// Build a user-friendly label for the copy files action based on selection count.
fn copy_files_label(count: usize) -> String {
    match count {
        0 | 1 => "Copy Image".into(),
        n => format!("Copy {} Items", n),
    }
}
