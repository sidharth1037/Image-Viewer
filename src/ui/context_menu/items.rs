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
            label: "Rotate Clockwise",
            shortcut_hint: Some("Shift+R"),
            enabled: has_image,
            id: "rotate_clockwise",
        }));

        entries.push(ContextMenuEntry::Item(ContextMenuItem {
            icon: icons::FLOPPY_DISK,
            label: "Save with Adjustments",
            shortcut_hint: Some("Ctrl+Shift+S"),
            enabled: has_adjustments,
            id: "save_adjustments",
        }));

        entries.push(ContextMenuEntry::Separator);
    }

    entries.push(ContextMenuEntry::Item(ContextMenuItem {
        icon: icons::FOLDER_OPEN,
        label: "Reveal in Explorer",
        shortcut_hint: Some("Ctrl+Shift+E"),
        enabled: has_file,
        id: "reveal_in_explorer",
    }));

    entries.push(ContextMenuEntry::Item(ContextMenuItem {
        icon: icons::CLIPBOARD,
        label: "Copy Path",
        shortcut_hint: None,
        enabled: has_file,
        id: "copy_path",
    }));

    entries.push(ContextMenuEntry::Separator);

    entries.push(ContextMenuEntry::Item(ContextMenuItem {
        icon: icons::TRASH,
        label: "Delete Permanently",
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
    let has_selection = app
        .workspace
        .playlist_grid
        .as_ref()
        .is_some_and(|grid| grid.selection.has_selection());

    let mut entries = Vec::new();

    entries.push(ContextMenuEntry::Item(ContextMenuItem {
        icon: icons::IMAGE,
        label: "Open Image",
        shortcut_hint: None,
        enabled: has_selection,
        id: "open_image",
    }));

    entries.push(ContextMenuEntry::Item(ContextMenuItem {
        icon: icons::FOLDER_OPEN,
        label: "Reveal in Explorer",
        shortcut_hint: Some("Ctrl+Shift+E"),
        enabled: has_selection,
        id: "reveal_in_explorer",
    }));

    entries.push(ContextMenuEntry::Item(ContextMenuItem {
        icon: icons::CLIPBOARD,
        label: "Copy Path",
        shortcut_hint: None,
        enabled: has_selection,
        id: "copy_path",
    }));

    entries.push(ContextMenuEntry::Separator);

    entries.push(ContextMenuEntry::Item(ContextMenuItem {
        icon: icons::CHECKS,
        label: "Select All",
        shortcut_hint: Some("Ctrl+A"),
        enabled: has_items,
        id: "select_all",
    }));

    entries.push(ContextMenuEntry::Separator);

    entries.push(ContextMenuEntry::Item(ContextMenuItem {
        icon: icons::TRASH,
        label: "Delete Permanently",
        shortcut_hint: Some("Shift+Del"),
        enabled: has_selection,
        id: "delete",
    }));

    entries
}

/// Build context menu entries for the **DuplicateFinder** view.
pub fn duplicate_items(app: &ImageApp) -> Vec<ContextMenuEntry> {
    let has_selection = app
        .workspace
        .duplicate_finder
        .as_ref()
        .is_some_and(|dup| {
            dup.active_scan()
                .groups
                .iter()
                .any(|g| g.selection.has_selection())
        });

    let mut entries = Vec::new();

    entries.push(ContextMenuEntry::Item(ContextMenuItem {
        icon: icons::IMAGE,
        label: "Open Image",
        shortcut_hint: None,
        enabled: has_selection,
        id: "open_image",
    }));

    entries.push(ContextMenuEntry::Item(ContextMenuItem {
        icon: icons::FOLDER_OPEN,
        label: "Reveal in Explorer",
        shortcut_hint: Some("Ctrl+Shift+E"),
        enabled: has_selection,
        id: "reveal_in_explorer",
    }));

    entries.push(ContextMenuEntry::Item(ContextMenuItem {
        icon: icons::CLIPBOARD,
        label: "Copy Path",
        shortcut_hint: None,
        enabled: has_selection,
        id: "copy_path",
    }));

    entries.push(ContextMenuEntry::Separator);

    entries.push(ContextMenuEntry::Item(ContextMenuItem {
        icon: icons::TRASH,
        label: "Delete Permanently",
        shortcut_hint: Some("Shift+Del"),
        enabled: has_selection,
        id: "delete",
    }));

    entries
}
