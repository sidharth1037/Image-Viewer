/// Self-contained saturation adjustment.
///
/// `value = 0.0` is neutral (no change).
/// Positive values increase color intensity, negative values desaturate.

use super::scalar;

#[derive(Clone)]
pub struct SaturationAdjustment {
    pub value: f32,
}

impl Default for SaturationAdjustment {
    fn default() -> Self {
        Self { value: 0.0 }
    }
}

impl SaturationAdjustment {
    pub const STEP: f32 = 0.01;
    pub const MIN: f32 = -1.0;
    pub const MAX: f32 = 2.0;

    pub fn is_neutral(&self) -> bool {
        scalar::is_neutral_value(self.value)
    }

    pub fn reset(&mut self) {
        self.value = 0.0;
    }

    pub fn adjust_by(&mut self, delta: f32) -> bool {
        scalar::apply_delta_clamped(&mut self.value, delta, Self::MIN, Self::MAX)
    }

    pub fn apply(&self, pixels: &mut [u8]) {
        if self.is_neutral() {
            return;
        }

        let factor = 1.0 + self.value;

        for chunk in pixels.chunks_exact_mut(4) {
            let r = chunk[0] as f32;
            let g = chunk[1] as f32;
            let b = chunk[2] as f32;

            let gray = 0.299 * r + 0.587 * g + 0.114 * b;

            let nr = (gray + (r - gray) * factor).clamp(0.0, 255.0);
            let ng = (gray + (g - gray) * factor).clamp(0.0, 255.0);
            let nb = (gray + (b - gray) * factor).clamp(0.0, 255.0);

            chunk[0] = (nr + 0.5) as u8;
            chunk[1] = (ng + 0.5) as u8;
            chunk[2] = (nb + 0.5) as u8;
        }
    }
}
