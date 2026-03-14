pub struct ViewerState {
    pub is_fullscreen: bool,
}

impl ViewerState {
    pub fn new() -> Self {
        Self {
            // This sets the initial state when the app first opens
            is_fullscreen: false,
        }
    }
}