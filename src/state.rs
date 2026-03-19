pub struct ViewerState {
    pub is_fullscreen: bool,
    pub current_file_name: String,
}

impl ViewerState {
    pub fn new() -> Self {
        Self {
            // This sets the initial state when the app first opens
            is_fullscreen: false,
            current_file_name: String::new(),
        }
    }
}