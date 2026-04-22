use crate::app::ImageApp;
use crate::ui::dialogs::confirmation_dialog::{
    self, ConfirmationDialogAction, ConfirmationDialogSpec,
};
use eframe::egui;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeleteFileDialogAction {
    Cancel,
    Confirm,
}

pub fn render(
    app: &mut ImageApp,
    ctx: &egui::Context,
    backdrop_rect: egui::Rect,
    dialog_center: Option<egui::Pos2>,
) -> Option<DeleteFileDialogAction> {
    if !app.show_delete_file_dialog {
        return None;
    }

    let target = app
        .delete_file_dialog_target
        .as_ref()
        .or(app.workspace.active_view().current_file_path.as_ref());

    let Some(path) = target else {
        return None;
    };

    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned());

    let message = format!(
        "Permanently delete this file?\n\n{}\n\nThis action cannot be undone.",
        file_name
    );

    let spec = ConfirmationDialogSpec {
        id_source: "delete_file_confirmation_dialog",
        title: "Delete File",
        message: &message,
        cancel_label: "Cancel",
        confirm_label: "Delete",
    };

    match confirmation_dialog::render(ctx, &spec, app.delete_file_dialog_selection, backdrop_rect, dialog_center) {
        Some(ConfirmationDialogAction::Cancel) => Some(DeleteFileDialogAction::Cancel),
        Some(ConfirmationDialogAction::Confirm) => Some(DeleteFileDialogAction::Confirm),
        None => None,
    }
}
