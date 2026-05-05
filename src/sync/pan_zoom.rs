use crate::app::ImageApp;
use crate::state::ViewerState;
use eframe::egui::Vec2;

#[derive(Clone, Copy)]
pub struct PanZoomSnapshot {
    pub auto_fit: bool,
    pub scale: f32,
    pub pan: Vec2,
    pub target_scale: Option<f32>,
    pub target_pan: Option<Vec2>,
    pub reset_start_time: Option<f64>,
}

impl PanZoomSnapshot {
    pub fn from_state(state: &ViewerState) -> Self {
        Self {
            auto_fit: state.auto_fit,
            scale: state.scale,
            pan: state.pan,
            target_scale: state.target_scale,
            target_pan: state.target_pan,
            reset_start_time: state.reset_start_time,
        }
    }

    pub fn differs_from(&self, other: &PanZoomSnapshot) -> bool {
        self.auto_fit != other.auto_fit
            || self.scale != other.scale
            || vec2_differs(self.pan, other.pan)
            || self.target_scale != other.target_scale
            || option_vec2_differs(self.target_pan, other.target_pan)
            || self.reset_start_time != other.reset_start_time
    }

    pub fn apply_to(&self, target: &mut ViewerState) {
        target.auto_fit = self.auto_fit;
        target.scale = self.scale;
        target.pan = self.pan;
        target.target_scale = self.target_scale;
        target.target_pan = self.target_pan;
        target.reset_start_time = self.reset_start_time;
    }
}

pub fn can_enable_sync(app: &ImageApp) -> bool {
    app.workspace.is_split() && aspect_ratio_mismatch_reason(app).is_none()
}

pub fn aspect_ratio_mismatch_reason(app: &ImageApp) -> Option<String> {
    if !app.workspace.is_split() {
        return Some("Split view is off".to_string());
    }

    let right_view = &app.workspace.views[0];
    let left_view = &app.workspace.views[1];

    let left_ratio = aspect_ratio_after_rotation(left_view);
    let right_ratio = aspect_ratio_after_rotation(right_view);

    match (left_ratio, right_ratio) {
        (Some(left), Some(right)) => {
            if left == right {
                None
            } else {
                Some("Aspect ratios differ".to_string())
            }
        }
        _ => Some("Images not loaded on both sides".to_string()),
    }
}

pub fn aspect_ratio_after_rotation(state: &ViewerState) -> Option<(u32, u32)> {
    let (width, height) = state.image_resolution?;
    if width == 0 || height == 0 {
        return None;
    }

    let turns = state.rotation_quarter_turns % 4;
    let (w, h) = if turns % 2 == 1 { (height, width) } else { (width, height) };

    Some(reduce_ratio(w, h))
}

fn reduce_ratio(width: u32, height: u32) -> (u32, u32) {
    let gcd = gcd_u32(width, height);
    (width / gcd, height / gcd)
}

fn gcd_u32(mut a: u32, mut b: u32) -> u32 {
    while b != 0 {
        let r = a % b;
        a = b;
        b = r;
    }
    a.max(1)
}

fn vec2_differs(a: Vec2, b: Vec2) -> bool {
    a.x != b.x || a.y != b.y
}

fn option_vec2_differs(a: Option<Vec2>, b: Option<Vec2>) -> bool {
    match (a, b) {
        (Some(left), Some(right)) => vec2_differs(left, right),
        (None, None) => false,
        _ => true,
    }
}
