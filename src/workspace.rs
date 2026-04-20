use crate::state::ViewerState;

pub struct Workspace {
    pub views: Vec<ViewerState>,
    pub active_view_index: usize,
}

impl Workspace {
    pub fn new(initial_view: ViewerState) -> Self {
        Self {
            views: vec![initial_view],
            active_view_index: 0,
        }
    }

    pub fn active_view(&self) -> &ViewerState {
        &self.views[self.active_view_index]
    }

    pub fn active_view_mut(&mut self) -> &mut ViewerState {
        &mut self.views[self.active_view_index]
    }

    pub fn is_split(&self) -> bool {
        self.views.len() > 1
    }

    pub fn toggle_split(&mut self, ctx: &eframe::egui::Context) {
        if self.is_split() {
            // Disable split: keep the primary view (index 0), evict the cloned view (index 1).
            self.views.truncate(1);
            self.active_view_index = 0;
        } else {
            // Enable split: clone the primary view into a second pane.
            let cloned_view = self.views[0].clone_for_compare(ctx);
            self.views.push(cloned_view);
        }
    }
}
