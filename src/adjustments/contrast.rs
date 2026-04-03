/// Self-contained contrast adjustment.
///
/// `value = 0.0` is neutral (no change).
/// Positive values increase contrast, negative values reduce contrast.
///
/// Formula (around mid-gray):
/// `output = (input - 128) * (1 + value) + 128`

use super::scalar;

pub struct ContrastAdjustment {
    pub value: f32,
}

impl Default for ContrastAdjustment {
    fn default() -> Self {
        Self { value: 0.0 }
    }
}

impl ContrastAdjustment {
    /// Step size for keyboard increment/decrement.
    pub const STEP: f32 = 0.01;

    /// Practical limits for contrast multiplier (`1.0 + value`).
    pub const MIN: f32 = -0.95;
    pub const MAX: f32 = 2.0;

    /// Returns true if contrast is at the neutral (identity) value.
    pub fn is_neutral(&self) -> bool {
        scalar::is_neutral_value(self.value)
    }

    /// Resets contrast to the neutral value.
    pub fn reset(&mut self) {
        self.value = 0.0;
    }

    /// Applies a signed delta to contrast and clamps it to valid bounds.
    pub fn adjust_by(&mut self, delta: f32) -> bool {
        scalar::apply_delta_clamped(&mut self.value, delta, Self::MIN, Self::MAX)
    }

    /// Builds a 256-entry LUT for current contrast to avoid per-pixel math.
    fn build_lut(&self) -> [u8; 256] {
        let mut lut = [0u8; 256];
        let factor = 1.0 + self.value;

        for i in 0..256 {
            let centered = i as f32 - 128.0;
            let adjusted = centered * factor + 128.0;
            let clamped = adjusted.clamp(0.0, 255.0);
            lut[i] = (clamped + 0.5) as u8;
        }

        lut
    }

    /// Applies contrast adjustment to an RGBA pixel buffer in-place.
    /// Only R, G, B channels are modified; alpha is left untouched.
    pub fn apply(&self, pixels: &mut [u8]) {
        if self.is_neutral() {
            return;
        }

        let lut = self.build_lut();

        for chunk in pixels.chunks_exact_mut(4) {
            chunk[0] = lut[chunk[0] as usize];
            chunk[1] = lut[chunk[1] as usize];
            chunk[2] = lut[chunk[2] as usize];
        }
    }
}
